mod agent;
mod bar;
#[cfg(test)]
mod bar_tests;
mod bus;
mod capture;
mod config_watch;
mod crash;
#[cfg(feature = "test-hooks")]
mod inject;
mod live_settings;
mod look;
mod notices;
mod pickers;
mod recorder;
mod recording;
mod recovery;
mod routes;
mod shutter;
mod tray;
mod windows;
#[cfg(test)]
mod wiring_tests;

use anyhow::{Context as _, Result, bail};
use es_app::capture_coordinator::{CaptureCoordinator, CaptureKind};
use es_app::deck::{Deck, DeckPolicy};
use es_app::dispatcher::RouteDispatcher;
use es_app::events::{EventHandlers, PlatformEvent, spawn_pump};
use es_app::history::History;
use es_app::pipeline::{self, PipelineDeps, Reporting};
use es_app::platform_ui::{
    BackendClipboardSink, LayerShellSurfaces, PinnedOutput, PinnedOutputSource, SelectMode,
    SelectionFlow as _, ToastSurface,
};
use es_app::services::Services;
use es_app::settings_model::SettingsModel;
use es_app::toast::install_toasts;
use es_app::windows::{WindowKey, WindowKind, Windows};
use es_backend::{Backend, Probe};
use es_desktop::dbus::Endpoint;
use es_desktop::{Labels, Notification, NotificationId, Notifier};
use es_model::platform::OutputId;
use es_overlays::capture::{FlowOptions, LayerShellSelectionFlow};
use es_overlays::deck::view::{DeckSettings, DeckSurface};
use es_overlays::pin::Pins;
use es_shell::gallery::DesktopFiles;
use es_store::{ConfigFile, Paths};
use gpui::{App, AppContext as _, Global, QuitMode, Task};
use recovery::reopen_recovered;
use std::rc::Rc;
use std::sync::Arc;
use tracing_subscriber::EnvFilter;
use tracing_subscriber::layer::SubscriberExt as _;
use tracing_subscriber::util::SubscriberInitExt as _;
use windows::Wiring;

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
    _pin_watch: gpui::Subscription,
    _look: gpui::Subscription,
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

fn init_logging(
    paths: &Paths,
    ring: &crash::LogRing,
) -> Option<tracing_appender::non_blocking::WorkerGuard> {
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
        .with(
            tracing_subscriber::fmt::layer()
                .with_ansi(false)
                .with_writer(ring.clone()),
        )
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
    let ring = crash::LogRing::default();
    let _log = init_logging(&paths, &ring);
    let crash_context = crash::Context {
        version: env!("EASYSHOT_VERSION").to_owned(),
        features: crate::enabled_features(),
        providers: Default::default(),
    };
    crash::install_hook(crash_context.clone(), ring, paths.crash_dir());
    if let Err(e) = es_shell::onboarding::prepare_for_launch(&paths) {
        tracing::warn!(error = %e, "first-launch check");
    }
    let config = ConfigFile::load(&paths.config_file()).context("reading the settings")?;
    let (backend, test_output) = detect_backend()?;
    crash_context.set_providers(
        backend
            .report
            .entries
            .iter()
            .map(|e| {
                (
                    e.capability.to_owned(),
                    e.chosen.unwrap_or("none").to_owned(),
                )
            })
            .collect(),
    );
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
    let settings = cx.new(|_| SettingsModel::new(config));
    let look = look::install(&settings, backend.appearance.as_deref(), cx);
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
    cx.set_global(services.images.clone());
    cx.set_global(services);

    let pinned = PinnedOutput::default();
    let apply_pin = {
        let pinned = pinned.clone();
        move |c: &es_model::Config| {
            pinned.set(c.capture.selector_follows_pointer, &c.capture.pinned_output)
        }
    };
    apply_pin(&cfg);
    let pin_watch = cx.observe(&settings, {
        let apply_pin = apply_pin.clone();
        move |s, cx| apply_pin(s.read(cx).config())
    });
    let source = Arc::new(PinnedOutputSource::new(backend.clone(), pinned));
    let surfaces = Rc::new(LayerShellSurfaces::new(source, test_output));
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

    let recorder = recorder::for_backend(backend.clone());
    let recorder_missing = recorder("auto").is_err();
    let tray_feed = tray::TrayFeed::new(
        {
            let backend = backend.clone();
            Rc::new(move |input| {
                if let Some(t) = &backend.tray {
                    es_shell::tray::show(t.as_ref(), input);
                }
            })
        },
        {
            let pins = pins.clone();
            Rc::new(move || pins.count() > 0)
        },
        history.clone(),
        settings.clone(),
        paths.videos_dir(),
        recorder_missing,
    );
    pins.set_on_change({
        let feed = tray_feed.clone();
        Rc::new(move |cx| feed.refresh(cx))
    });
    let tray_subscriptions = tray_feed.follow(&deck, cx);

    let test_mode = test_mode_requested();
    let sound = shutter::player(test_mode);
    let files: Arc<dyn es_shell::gallery::GalleryFiles> =
        Arc::new(DesktopFiles::new(Endpoint::session()));
    let wiring = Wiring::new(
        paths.clone(),
        settings.clone(),
        history.clone(),
        clipboard.clone(),
        if test_mode {
            Rc::new(pickers::Declining)
        } else {
            Rc::new(pickers::PortalChooser(backend.clone()))
        },
        files.clone(),
        if test_mode {
            pickers::declining_folder_picker()
        } else {
            pickers::folder_picker()
        },
        backend.caps,
        es_shell::settings::from_backend(backend.clone()),
        {
            let backend = backend.clone();
            Rc::new(move || backend.capture.outputs())
        },
        Some(surfaces.clone()),
        windows.clone(),
    );
    wiring.register();
    if test_mode {
        wiring.set_wallpaper_chooser(Rc::new(pickers::Declining));
    }
    wiring.set_bar_sources(bar::BarSources {
        windows: {
            let backend = backend.clone();
            Rc::new(move || backend.windows.snapshot())
        },
        devices: bar::devices_from(if test_mode {
            Rc::new(bar::NoAudio)
        } else {
            Rc::new(es_record::audio::SystemRunner)
        }),
        recorder_missing,
    });

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
    let area_flow = flow.clone();
    coordinator.set_scroll_stop(flow.scroll_stop());
    coordinator.set_selection_flow(Rc::new(Reporting(flow)));
    coordinator.set_direct_capture(Rc::new(Reporting(Rc::new(capture::Direct::new(
        backend.clone(),
        surfaces.clone(),
        paths.clone(),
    )))));
    coordinator.set_captured(capture::remember_target(paths.clone(), backend.clone()));
    let timer = settings.clone();
    coordinator.set_countdown(es_overlays::countdown::hook(
        surfaces.clone(),
        Rc::new(move |cx| timer.read(cx).config().capture.self_timer),
    ));
    let pipeline_deps = Rc::new(PipelineDeps {
        paths: paths.clone(),
        settings: settings.clone(),
        history: history.clone(),
        deck: deck.clone(),
        clipboard: clipboard.clone(),
        recognize: pipeline::tesseract(),
        pin: capture::pinner(pins.clone()),
        open_editor: {
            let wiring = wiring.clone();
            Rc::new(move |file, cx| wiring.open_editor_for(&file, cx))
        },
    });
    let screenshots = pipeline::pipeline(pipeline_deps.clone());
    let recordings = recording::Recordings::new(recording::Deps {
        settings: settings.clone(),
        paths: paths.clone(),
        deck: deck.clone(),
        recorder,
        recorder_missing,
        outputs: {
            let backend = backend.clone();
            Rc::new(move || backend.capture.outputs())
        },
        ui: Rc::new(recording::LayerUi {
            surfaces: surfaces.clone(),
            tray: tray_update(tray_feed.clone()),
            notify: notice(backend.clone()),
            paths: paths.clone(),
        }),
        clock: Arc::new(es_record::SystemClock::default()),
        runner: Arc::new(es_record::project::SystemRunner),
        countdown: recording::ui::countdown(surfaces.clone()),
        select_area: Rc::new(move |cx| area_flow.run(SelectMode::Region, cx)),
        input: recording::input::for_system(Arc::new(es_record::SystemClock::default()), backend),
    });
    wiring.set_recordings(recordings.clone());
    let gate = recordings.clone();
    coordinator.set_gate(Rc::new(move |request, cx| match request.kind {
        CaptureKind::Recording | CaptureKind::RecordingArea => gate.admit(cx),
        _ => Ok(()),
    }));
    let rec = recordings.clone();
    let shutter_settings = settings.clone();
    coordinator.set_pipeline(Rc::new(move |request, selection, cx| match request.kind {
        CaptureKind::Recording | CaptureKind::RecordingArea => {
            rec.with_selection(request.kind, selection, cx)
        }
        _ => {
            shutter::play_after_capture(&shutter_settings, sound.as_ref(), cx);
            screenshots(request, selection, cx)
        }
    }));
    coordinator.set_before_capture(bar::close_before_capture(&windows));
    let dispatcher = RouteDispatcher::new(jobs.clone(), coordinator, windows);
    wiring.set_dispatcher(dispatcher.clone());
    agent::capture::install(&dispatcher, &jobs, backend, &surfaces, &pipeline_deps);
    let action_deps = routes::ActionDeps {
        deck: deck.clone(),
        history: history.clone(),
        wiring: wiring.clone(),
        pins: pins.clone(),
        paths: paths.clone(),
        recordings: recordings.clone(),
    };
    let mut subscriptions = vec![
        routes::deck_events(&action_deps, clipboard, cx),
        routes::video_cards(&action_deps, cx),
        live_settings::install(&settings, &deck, &deck_surface, &history, cx),
    ];
    subscriptions.extend(tray_subscriptions);
    dispatcher.set_action_handler(routes::actions(action_deps));

    let crash_notice = Rc::new(std::cell::RefCell::new(crash::announce(
        paths,
        &BackendNotifier(backend.clone()),
        &labels(),
    )));
    let editor = wiring.clone();
    let handlers = EventHandlers {
        notification_action: Some(Rc::new(move |(id, action), cx| {
            crash::on_action(&crash_notice, &crash::open_with_default_app, id, &action);
            let edit = |file: &std::path::Path, cx: &mut App| {
                if let Err(e) = editor.open_editor_for(file, cx) {
                    tracing::warn!(error = %e, "edit from a notification");
                }
            };
            let answers = notices::Answers {
                open: &crash::open_with_default_app,
                files: files.as_ref(),
                edit: &edit,
            };
            notices::on_action(id, &action, &answers, cx);
        })),
        appearance: Some({
            let settings = settings.clone();
            Rc::new(move |event, cx| look::desktop_changed(event, &settings, cx))
        }),
        #[cfg(feature = "test-hooks")]
        test_inject: test_mode.then(|| {
            Rc::new(|text: String, cx: &mut App| inject::dispatch(&text, cx))
                as Rc<dyn Fn(String, &mut App)>
        }),
        ..Default::default()
    };
    let (events_tx, events_rx) = async_channel::unbounded::<PlatformEvent>();
    let mut tasks = vec![spawn_pump(
        events_rx,
        dispatcher.clone(),
        settings.clone(),
        handlers,
        cx,
    )];
    tasks.extend(config_watch::install(&settings, cx));
    tasks.extend(bus::platform_sources(backend, &tray_feed, &events_tx, cx));
    let bridge = bus::Bridge::start(backend, events_tx, dispatcher, jobs, finished, cx)?;

    if let Err(e) = es_store::temp::purge(paths, std::time::SystemTime::now()) {
        tracing::warn!(error = %e, "temporary files not purged");
    }
    let recovered = recovery::run(paths, &deck, &recordings, cx);
    reopen_recovered(&wiring, recovered, cx);
    if first_launch_pending(paths)
        && let Err(e) = wiring
            .windows
            .open(WindowKind::Onboarding, WindowKey::Single, cx)
    {
        tracing::warn!(error = %e, "onboarding");
    }
    if (cfg.general.show_capture_bar_at_launch || backend.tray.is_none())
        && !test_mode
        && let Err(e) = wiring
            .windows
            .open(WindowKind::CaptureBar, WindowKey::Single, cx)
    {
        tracing::warn!(error = %e, "capture bar at launch");
    }
    cx.set_global(Daemon {
        _surfaces: surfaces,
        _deck_surface: deck_surface,
        _pins: pins,
        _bus: Some(bridge),
        _tasks: tasks,
        _subscriptions: subscriptions,
        _pin_watch: pin_watch,
        _look: look,
    });
    tracing::info!("easyshot daemon ready");
    Ok(())
}

fn first_launch_pending(paths: &Paths) -> bool {
    !test_mode_requested()
        && es_store::state::open(paths)
            .is_ok_and(|state| es_shell::onboarding::should_present(&state.value))
}

fn tray_update(feed: Rc<tray::TrayFeed>) -> recording::ui::TrayUpdate {
    Rc::new(move |state, cx| feed.set_recording(state, cx))
}

fn notice(backend: Arc<Backend>) -> recording::ui::Notify {
    Rc::new(move |title| {
        let sent = backend.notifier.notify(Notification {
            title,
            body: String::new(),
            icon: None,
            image: None,
            actions: Vec::new(),
            urgency: es_desktop::Urgency::Normal,
            timeout: Some(std::time::Duration::from_secs(3)),
        });
        if let Err(e) = sent {
            tracing::warn!(error = %e, "recording notice");
        }
    })
}
