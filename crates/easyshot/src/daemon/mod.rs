mod bus;
mod capture;
mod recovery;
mod routes;

use anyhow::{Context as _, Result, bail};
use es_app::capture_coordinator::CaptureCoordinator;
use es_app::deck::{Deck, DeckPolicy};
use es_app::dispatcher::RouteDispatcher;
use es_app::events::{EventHandlers, PlatformEvent, spawn_pump};
use es_app::history::History;
use es_app::pipeline::{self, PipelineDeps, Reporting};
use es_app::platform_ui::{BackendClipboardSink, LayerShellSurfaces, ToastSurface};
use es_app::services::Services;
use es_app::settings_model::SettingsModel;
use es_app::toast::install_toasts;
use es_app::windows::Windows;
use es_backend::{Backend, Probe};
use es_desktop::{Labels, Notification, NotificationId, Notifier};
use es_model::platform::OutputId;
use es_overlays::capture::{FlowOptions, LayerShellSelectionFlow};
use es_overlays::deck::view::{DeckSettings, DeckSurface};
use es_overlays::pin::Pins;
use es_store::{ConfigFile, Paths};
use gpui::{App, AppContext as _, Global, QuitMode, Task};
use std::rc::Rc;
use std::sync::Arc;
use tracing_subscriber::EnvFilter;
use tracing_subscriber::layer::SubscriberExt as _;
use tracing_subscriber::util::SubscriberInitExt as _;

const MIN_RAYON_THREADS: usize = 6;
const LOG_FILES_KEPT: usize = 7;

/// Everything the running daemon keeps alive; dropping it stops the app's pumps.
struct Daemon {
    _surfaces: Rc<LayerShellSurfaces>,
    _deck_surface: Rc<DeckSurface>,
    _pins: Rc<Pins>,
    _bus: Option<bus::Bridge>,
    _tasks: Vec<Task<()>>,
    _subscriptions: Vec<gpui::Subscription>,
    recovered_edits: Vec<es_editor::session::journal::Recovered>,
}

impl Global for Daemon {}

/// `Notifier` is object-safe but `Backend` holds it boxed; toasts need a shared handle.
struct BackendNotifier(Arc<Backend>);

impl Notifier for BackendNotifier {
    fn notify(&self, n: Notification) -> es_desktop::Result<NotificationId> {
        self.0.notifier.notify(n)
    }
    fn actions(&self) -> async_channel::Receiver<(NotificationId, String)> {
        self.0.notifier.actions()
    }
    fn supports_actions(&self) -> bool {
        self.0.notifier.supports_actions()
    }
}

fn test_mode_requested() -> bool {
    std::env::var("EASYSHOT_TEST").as_deref() == Ok("1")
}

fn init_logging(paths: &Paths) -> Option<tracing_appender::non_blocking::WorkerGuard> {
    let filter = EnvFilter::try_from_env("EASYSHOT_LOG").unwrap_or_else(|_| EnvFilter::new("info"));
    let stderr = tracing_subscriber::fmt::layer().with_writer(std::io::stderr);
    let file = tracing_appender::rolling::Builder::new()
        .rotation(tracing_appender::rolling::Rotation::DAILY)
        .max_log_files(LOG_FILES_KEPT)
        .filename_prefix("easyshot")
        .filename_suffix("log")
        .build(paths.log_dir());
    let (file_layer, guard) = match file {
        Ok(appender) => {
            let (writer, guard) = tracing_appender::non_blocking(appender);
            let layer = tracing_subscriber::fmt::layer()
                .with_ansi(false)
                .with_writer(writer);
            (Some(layer), Some(guard))
        }
        Err(e) => {
            eprintln!("easyshot: log file unavailable: {e}");
            (None, None)
        }
    };
    let _ = tracing_subscriber::registry()
        .with(filter)
        .with(stderr)
        .with(file_layer)
        .try_init();
    guard
}

/// Half the cores, never fewer than six: pixel loops stop scaling well past that, and the rest
/// stays free for the UI, the compositor and a running recorder's encoder.
fn size_rayon_pool() {
    if std::env::var_os("RAYON_NUM_THREADS").is_some() {
        return;
    }
    let cores = std::thread::available_parallelism().map_or(MIN_RAYON_THREADS, |n| n.get());
    let threads = (cores / 2).max(MIN_RAYON_THREADS);
    let built = rayon::ThreadPoolBuilder::new()
        .num_threads(threads)
        .thread_name(|i| format!("es-pixels-{i}"))
        .build_global();
    match built {
        Ok(()) => tracing::info!(threads, "pixel thread pool"),
        Err(e) => tracing::warn!(error = %e, "pixel thread pool already built"),
    }
}

fn labels() -> Labels {
    Arc::new(|id: &str| {
        let shell = es_shell::strings::loader();
        let app = es_app::strings::loader();
        if shell.has(id) {
            shell.get(id)
        } else if app.has(id) {
            app.get(id)
        } else {
            id.to_owned()
        }
    })
}

fn detect_backend() -> Result<(Arc<Backend>, Option<OutputId>)> {
    if test_mode_requested() && !cfg!(feature = "test-hooks") {
        bail!("EASYSHOT_TEST=1 needs a build with the test-hooks feature");
    }
    let mut probe = Probe::gather().context("probing the desktop")?;
    #[cfg(feature = "test-hooks")]
    let test_output = probe.test.as_ref().map(|t| t.output.clone());
    #[cfg(not(feature = "test-hooks"))]
    let test_output = None;
    if test_mode_requested() && test_output.is_none() {
        bail!("test mode needs EASYSHOT_TEST_OUTPUT");
    }
    probe.labels = Some(labels());
    let backend = Arc::new(Backend::detect(&probe));
    for line in &backend.report.errors {
        tracing::warn!(probe = %line, "desktop probe");
    }
    tracing::info!(caps = ?backend.caps, "backend");
    #[cfg(feature = "test-hooks")]
    wake_test_pointer(&backend);
    Ok((backend, test_output))
}

#[derive(Debug, Default)]
pub struct DaemonArgs;

// A headless compositor has no pointer until a virtual one exists, and GPUI binds its pointer only then.
#[cfg(feature = "test-hooks")]
fn wake_test_pointer(backend: &Backend) {
    let (Some(_), Some(input)) = (&backend.test, &backend.virtual_input) else {
        return;
    };
    let Some(output) = backend.capture.outputs().into_iter().next() else {
        return;
    };
    let at = es_geom::Point::new(output.logical.x0 + 10., output.logical.y0 + 10.);
    if let Err(e) = input.scroll(at, 0.0) {
        tracing::warn!(error = %e, "test virtual pointer");
    }
}

pub fn run(_args: DaemonArgs) -> Result<()> {
    let paths = Paths::from_xdg().context("finding the XDG directories")?;
    paths
        .ensure_all()
        .context("creating easyshot's directories")?;
    let _log = init_logging(&paths);
    let config = ConfigFile::load(&paths.config_file()).context("reading the settings")?;
    let (backend, test_output) = detect_backend()?;
    size_rayon_pool();
    let (failed_tx, failed_rx) = std::sync::mpsc::channel::<String>();
    gpui_platform::application()
        .with_assets(es_ui::assets::Assets)
        .with_quit_mode(QuitMode::Explicit)
        .run(move |cx: &mut App| {
            if let Err(e) = start(&paths, config, &backend, test_output, cx) {
                tracing::error!(error = %format!("{e:#}"), "daemon start failed");
                let _ = failed_tx.send(format!("{e:#}"));
                cx.quit();
            }
        });
    match failed_rx.try_recv() {
        Ok(reason) => bail!(reason),
        Err(_) => Ok(()),
    }
}

fn start(
    paths: &Paths,
    config: ConfigFile,
    backend: &Arc<Backend>,
    test_output: Option<OutputId>,
    cx: &mut App,
) -> Result<()> {
    es_ui::hello::init(cx);
    let settings = cx.new(|_| SettingsModel::new(config));
    if let Some(e) = settings.read(cx).load_error() {
        tracing::warn!(error = %e, "settings file has an error; using defaults");
    }
    let cfg = settings.read(cx).config().clone();
    let history = es_app::history::History::open(paths, cfg.general.history_limit as usize)
        .or_else(|e| {
            tracing::warn!(error = %e, "history unreadable");
            History::open(&Paths::rooted(&paths.unstaged_dir()), 0)
        })
        .context("opening history")?;
    let history = cx.new(|_| history);
    let deck = cx.new(|_| Deck::new(DeckPolicy::from_settings(&cfg.overlay, &cfg.capture)));
    let windows = Windows::default();
    let (services, finished) = Services::new(
        backend.clone(),
        settings.clone(),
        history.clone(),
        deck.clone(),
        windows.clone(),
        paths.clone(),
    );
    let jobs = services.jobs.clone();
    cx.set_global(services);

    let surfaces = Rc::new(LayerShellSurfaces::new(backend.clone(), test_output));
    if let Err(e) = surfaces.warm_up(cx) {
        tracing::warn!(error = %e, "surface warm-up");
    }
    install_toasts(
        ToastSurface::new(surfaces.clone()),
        Arc::new(BackendNotifier(backend.clone())),
        cx,
    );
    let deck_surface = DeckSurface::attach(
        surfaces.clone(),
        &deck,
        DeckSettings::from_settings(&cfg.overlay),
        cx,
    );
    let clipboard = Rc::new(BackendClipboardSink::new(backend.clone(), paths.clone()));
    let pins = Rc::new(Pins::new(
        surfaces.clone(),
        capture::copy_file(clipboard.clone()),
    ));

    let coordinator = CaptureCoordinator::new(jobs.clone());
    let opts = settings.clone();
    let flow = Rc::new(
        LayerShellSelectionFlow::new(surfaces.clone(), backend.clone())
            .with_options(Rc::new(move |cx| {
                let c = &opts.read(cx).config().capture;
                FlowOptions {
                    capture_on_release: c.capture_on_release,
                    preselect_previous: c.preselect_previous,
                }
            }))
            .with_last_region(
                capture::recall(paths.clone()),
                capture::remember(paths.clone()),
            ),
    );
    coordinator.set_scroll_stop(flow.scroll_stop());
    coordinator.set_selection_flow(Rc::new(Reporting(flow)));
    coordinator.set_direct_capture(Rc::new(Reporting(Rc::new(capture::Direct::new(
        backend.clone(),
        surfaces.clone(),
        paths.clone(),
    )))));
    let timer = settings.clone();
    coordinator.set_countdown(es_overlays::countdown::hook(
        surfaces.clone(),
        Rc::new(move |cx| timer.read(cx).config().capture.self_timer),
    ));
    coordinator.set_pipeline(pipeline::pipeline(Rc::new(PipelineDeps {
        paths: paths.clone(),
        settings: settings.clone(),
        history: history.clone(),
        deck: deck.clone(),
        clipboard: clipboard.clone(),
        recognize: pipeline::tesseract(),
        pin: capture::pinner(pins.clone()),
        open_editor: Rc::new(|file, _| {
            tracing::error!(file = %file.display(), "the editor window is not wired yet");
            Err(es_app::errors::Error::Unsupported)
        }),
    })));

    routes::register_windows(&windows);
    let dispatcher = RouteDispatcher::new(jobs.clone(), coordinator, windows);
    dispatcher.set_action_handler(routes::actions(deck.clone()));
    let subscriptions = vec![routes::deck_events(
        &deck,
        paths.clone(),
        clipboard,
        pins.clone(),
        cx,
    )];

    let (events_tx, events_rx) = async_channel::unbounded::<PlatformEvent>();
    let mut tasks = vec![spawn_pump(
        events_rx,
        dispatcher.clone(),
        settings.clone(),
        EventHandlers::default(),
        cx,
    )];
    tasks.extend(bus::platform_sources(backend, &history, &events_tx, cx));
    let bridge = bus::Bridge::start(backend, events_tx, dispatcher, jobs, finished, cx)?;

    let recovered_edits = recovery::run(paths, &deck, cx);
    cx.set_global(Daemon {
        _surfaces: surfaces,
        _deck_surface: deck_surface,
        _pins: pins,
        _bus: Some(bridge),
        _tasks: tasks,
        _subscriptions: subscriptions,
        recovered_edits,
    });
    tracing::info!(
        pending_recovered_edits = cx.global::<Daemon>().recovered_edits.len(),
        "easyshot daemon ready"
    );
    Ok(())
}
