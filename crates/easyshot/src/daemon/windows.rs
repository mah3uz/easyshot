use super::bar::{BarSources, new_bar, start_recording};
use es_app::dispatcher::{HomeTab, RouteDispatcher};
use es_app::errors::{Error, Result};
use es_app::events::Origin;
use es_app::history::{History, HistoryEvent};
use es_app::platform_ui::{ClipboardSink, LayerShellSurfaces, SurfaceHandle as _, SurfaceRole};
use es_app::settings_model::SettingsModel;
use es_app::windows::{WindowHandle, WindowKey, WindowKind, Windows};
use es_backend::{BindHelper, Caps};
use es_editor::inspector::Section;
use es_editor::inspector::background::{BackgroundBody, WallpaperChooser};
use es_editor::session::io::{ExportChooser, IoConfig};
use es_editor::{EditorSession, EditorSource, EditorWindow};
use es_model::platform::OutputInfo;
use es_overlays::record::bar::{BAR, BarEvent};
use es_overlays::record::placement;
use es_protocol::{Route, SettingsPage};
use es_shell::gallery::{GalleryEvent, GalleryFiles};
use es_shell::home::{self, HomeDeps, HomeWindow, Tab};
use es_shell::onboarding::{self, Onboarding, OnboardingEvent, OnboardingInput};
use es_shell::settings::{self, SettingsDeps, SettingsWindowEvent};
use es_store::Paths;
use gpui::{
    AnyWindowHandle, App, AppContext as _, Bounds, Entity, Task, WeakEntity, WindowBounds,
    WindowDecorations, WindowOptions, point, px, size,
};
use gpui_component::{Root, TitleBar};
use std::cell::RefCell;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Arc;

pub type FolderPicker = Rc<dyn Fn(&mut App) -> Task<Option<PathBuf>>>;

static PRACTICE_IMAGE: &[u8] = include_bytes!("../../../../assets/practice.png");

/// What the daemon's windows need from the rest of the app.
pub struct Wiring {
    pub paths: Paths,
    pub settings: Entity<SettingsModel>,
    pub history: Entity<History>,
    pub clipboard: Rc<dyn ClipboardSink>,
    pub chooser: Rc<dyn ExportChooser>,
    pub files: Arc<dyn GalleryFiles>,
    pub folder: FolderPicker,
    pub caps: Caps,
    pub binds: Option<Arc<dyn BindHelper>>,
    pub outputs: Rc<dyn Fn() -> Vec<OutputInfo>>,
    pub surfaces: Option<Rc<LayerShellSurfaces>>,
    pub windows: Windows,
    dispatcher: RefCell<Option<RouteDispatcher>>,
    recordings: RefCell<Option<super::recording::Recordings>>,
    pub(super) bar_sources: RefCell<BarSources>,
    editors: RefCell<Vec<WeakEntity<EditorWindow>>>,
    home: RefCell<Option<WeakEntity<HomeWindow>>>,
    recoveries: RefCell<HashMap<PathBuf, PathBuf>>,
    wallpaper: RefCell<Option<Rc<dyn WallpaperChooser>>>,
}

struct Handle(AnyWindowHandle);

impl WindowHandle for Handle {
    fn is_open(&self, cx: &App) -> bool {
        self.0.is_open(cx)
    }
    fn focus(&self, cx: &mut App) {
        self.0.focus(cx)
    }
    fn close(&self, cx: &mut App) {
        WindowHandle::close(&self.0, cx)
    }
}

fn handle(window: impl Into<AnyWindowHandle>) -> Rc<dyn WindowHandle> {
    Rc::new(Handle(window.into()))
}

fn opened<T>(result: anyhow::Result<T>) -> Result<T> {
    result.map_err(|e| Error::Protocol(format!("cannot open the window: {e}")))
}

fn canonical(path: &Path) -> PathBuf {
    path.canonicalize().unwrap_or_else(|_| path.to_owned())
}

impl Wiring {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        paths: Paths,
        settings: Entity<SettingsModel>,
        history: Entity<History>,
        clipboard: Rc<dyn ClipboardSink>,
        chooser: Rc<dyn ExportChooser>,
        files: Arc<dyn GalleryFiles>,
        folder: FolderPicker,
        caps: Caps,
        binds: Option<Arc<dyn BindHelper>>,
        outputs: Rc<dyn Fn() -> Vec<OutputInfo>>,
        surfaces: Option<Rc<LayerShellSurfaces>>,
        windows: Windows,
    ) -> Rc<Self> {
        Rc::new(Self {
            paths,
            settings,
            history,
            clipboard,
            chooser,
            files,
            folder,
            caps,
            binds,
            outputs,
            surfaces,
            windows,
            dispatcher: RefCell::default(),
            recordings: RefCell::default(),
            bar_sources: RefCell::default(),
            editors: RefCell::default(),
            home: RefCell::default(),
            recoveries: RefCell::default(),
            wallpaper: RefCell::default(),
        })
    }

    pub fn set_recordings(&self, recordings: super::recording::Recordings) {
        *self.recordings.borrow_mut() = Some(recordings);
    }

    pub fn set_wallpaper_chooser(&self, chooser: Rc<dyn WallpaperChooser>) {
        *self.wallpaper.borrow_mut() = Some(chooser);
    }

    pub fn set_bar_sources(&self, sources: BarSources) {
        *self.bar_sources.borrow_mut() = sources;
    }

    pub fn set_dispatcher(&self, dispatcher: RouteDispatcher) {
        *self.dispatcher.borrow_mut() = Some(dispatcher);
    }

    pub fn register(self: &Rc<Self>) {
        let w = self.clone();
        self.windows
            .register(WindowKind::Home, Rc::new(move |_, cx| open_home(&w, cx)));
        let w = self.clone();
        self.windows.register(
            WindowKind::Editor,
            Rc::new(move |key, cx| open_editor(&w, key, cx)),
        );
        let w = self.clone();
        self.windows.register(
            WindowKind::Onboarding,
            Rc::new(move |_, cx| open_onboarding(&w, cx)),
        );
        let w = self.clone();
        self.windows.register(
            WindowKind::CaptureBar,
            Rc::new(move |_, cx| open_bar(&w, cx)),
        );
    }

    pub fn dispatch(&self, route: Route, cx: &mut App) {
        let dispatcher = self.dispatcher.borrow().clone();
        match dispatcher {
            Some(d) => {
                d.dispatch(route, Origin::Internal, cx);
            }
            None => tracing::warn!(%route, "no dispatcher yet"),
        }
    }

    pub fn open_editor_for(&self, file: &Path, cx: &mut App) -> Result<()> {
        self.windows
            .open(WindowKind::Editor, WindowKey::Path(file.to_owned()), cx)
    }

    /// Reopens an editor on the unsaved edits a journal holds.
    pub fn recover(&self, display: &Path, journal: PathBuf, cx: &mut App) -> Result<()> {
        self.recoveries
            .borrow_mut()
            .insert(canonical(display), journal);
        self.open_editor_for(display, cx)
    }

    pub fn show_home(&self, tab: HomeTab, cx: &mut App) {
        let home = self.home.borrow().as_ref().and_then(WeakEntity::upgrade);
        if let Some(home) = home {
            home.update(cx, |h, cx| match tab {
                HomeTab::Gallery => h.show(Tab::Gallery, cx),
                HomeTab::Settings(page) => h.show_settings(page.map(page_of), cx),
            });
        }
    }

    #[cfg(test)]
    pub fn home(&self) -> Option<Entity<HomeWindow>> {
        self.home.borrow().as_ref().and_then(WeakEntity::upgrade)
    }

    /// Asks every editor to close, and quits only when none of them objects.
    pub fn quit(&self, cx: &mut App) {
        let editors: Vec<_> = self
            .editors
            .borrow()
            .iter()
            .filter_map(WeakEntity::upgrade)
            .collect();
        cx.spawn(async move |cx| {
            for editor in editors {
                let closing = editor.update(cx, |w, cx| w.request_close(cx));
                if !closing.await {
                    return;
                }
            }
            cx.update(|cx| cx.quit());
        })
        .detach();
    }

    #[cfg(test)]
    pub fn editor_count(&self) -> usize {
        self.editors
            .borrow()
            .iter()
            .filter(|e| e.upgrade().is_some())
            .count()
    }
}

fn page_of(page: SettingsPage) -> settings::Page {
    use settings::Page;
    match page {
        SettingsPage::Home => Page::Home,
        SettingsPage::General => Page::General,
        SettingsPage::Capture => Page::Capture,
        SettingsPage::Overlay => Page::Overlay,
        SettingsPage::Recording => Page::Recording,
        SettingsPage::Shortcuts => Page::Shortcuts,
        SettingsPage::About => Page::About,
    }
}

fn open_home(w: &Rc<Wiring>, cx: &mut App) -> Result<Rc<dyn WindowHandle>> {
    let deps = HomeDeps {
        history: w.history.clone(),
        paths: w.paths.clone(),
        files: w.files.clone(),
        settings: SettingsDeps {
            settings: w.settings.clone(),
            history: Some(w.history.clone()),
            caps: w.caps,
            binds: w.binds.clone(),
            paths: w.paths.clone(),
            clipboard: Rc::new(settings::SystemClipboard),
            outputs: (w.outputs)().into_iter().map(|o| o.id.0).collect(),
            version: env!("EASYSHOT_VERSION").to_owned(),
            input_access: Rc::new(es_record::input::system_access),
        },
    };
    let window = opened(home::open(deps, cx))?;
    let view = window
        .read(cx)
        .ok()
        .and_then(|root| root.view().clone().downcast::<HomeWindow>().ok());
    if let Some(view) = view {
        *w.home.borrow_mut() = Some(view.downgrade());
        let (gallery, settings) = {
            let home = view.read(cx);
            (home.gallery().clone(), home.settings().clone())
        };
        let on_settings = w.clone();
        cx.subscribe(
            &settings,
            move |_, event: &SettingsWindowEvent, cx| match event {
                SettingsWindowEvent::Run(route) => on_settings.dispatch(route.clone(), cx),
                SettingsWindowEvent::ChooseSaveFolder => choose_save_folder(&on_settings, cx),
                SettingsWindowEvent::OpenRecent(file) => {
                    if let Err(e) = on_settings.open_editor_for(file, cx) {
                        tracing::warn!(error = %e, "open a recent capture");
                    }
                }
                SettingsWindowEvent::ShowOnboarding => {
                    if let Err(e) =
                        on_settings
                            .windows
                            .open(WindowKind::Onboarding, WindowKey::Single, cx)
                    {
                        tracing::warn!(error = %e, "onboarding");
                    }
                }
                SettingsWindowEvent::OpenUrl(url) => cx.open_url(url),
            },
        )
        .detach();
        let on_gallery = w.clone();
        cx.subscribe(&gallery, move |_, event: &GalleryEvent, cx| match event {
            GalleryEvent::Open(file) => cx.open_with_system(file),
            GalleryEvent::Edit(file) => {
                if let Err(e) = on_gallery.open_editor_for(file, cx) {
                    tracing::warn!(error = %e, "edit from the gallery");
                }
            }
        })
        .detach();
    }
    Ok(handle(window))
}

fn choose_save_folder(w: &Rc<Wiring>, cx: &mut App) {
    let picked = (w.folder)(cx);
    let settings = w.settings.clone();
    cx.spawn(async move |cx| {
        if let Some(dir) = picked.await {
            settings.update(cx, |s, cx| s.update(|c| c.general.save_dir = Some(dir), cx));
        }
    })
    .detach();
}

pub(super) fn editor_bounds(fullscreen: bool, bounds: Bounds<gpui::Pixels>) -> WindowBounds {
    if fullscreen {
        WindowBounds::Fullscreen(bounds)
    } else {
        WindowBounds::Windowed(bounds)
    }
}

fn open_editor(w: &Rc<Wiring>, key: &WindowKey, cx: &mut App) -> Result<Rc<dyn WindowHandle>> {
    let WindowKey::Path(file) = key else {
        return Err(Error::Unsupported);
    };
    let source = match w.recoveries.borrow_mut().remove(file) {
        Some(journal) => EditorSource::Recovery(journal),
        None => EditorSource::File(file.clone()),
    };
    let opening = EditorSession::open_with_style_paths(source, Some(w.paths.clone()), cx);
    let bounds = Bounds::new(point(px(0.), px(0.)), size(px(1080.), px(720.)));
    let options = WindowOptions {
        window_bounds: Some(editor_bounds(
            w.settings.read(cx).config().general.editor_opens_fullscreen,
            bounds,
        )),
        window_min_size: Some(size(px(980.), px(640.))),
        window_decorations: Some(WindowDecorations::Client),
        app_id: Some(es_protocol::EDITOR_APP_ID.into()),
        ..TitleBar::window_options()
    };
    let editor = Rc::new(RefCell::new(None::<Entity<EditorWindow>>));
    let slot = editor.clone();
    let window = opened(cx.open_window(options, move |window, cx| {
        let view = cx.new(|_| EditorWindow::loading());
        *slot.borrow_mut() = Some(view.clone());
        cx.new(|cx| Root::new(view, window, cx))
    }))?;
    let Some(editor) = editor.borrow_mut().take() else {
        return Err(Error::Internal("the editor window has no view".into()));
    };
    w.editors.borrow_mut().retain(|e| e.upgrade().is_some());
    w.editors.borrow_mut().push(editor.downgrade());
    let settings = w.settings.clone();
    let wallpaper = w.wallpaper.borrow().clone();
    let config = IoConfig {
        chooser: Some(w.chooser.clone()),
        clipboard: Some(w.clipboard.clone()),
        paths: Some(w.paths.clone()),
        settings: None,
    };
    cx.spawn(async move |cx| {
        let result = opening.await;
        cx.update(|cx| {
            if let Ok(session) = &result {
                session.io.update(cx, |io, _| io.configure(config));
            }
            let loaded = result.is_ok();
            editor.update(cx, |e, cx| e.loaded(result, cx));
            if loaded {
                finish_editor(&settings, wallpaper, &editor, cx);
            }
        });
    })
    .detach();
    Ok(handle(window))
}

fn finish_editor(
    settings: &Entity<SettingsModel>,
    wallpaper: Option<Rc<dyn WallpaperChooser>>,
    editor: &Entity<EditorWindow>,
    cx: &mut App,
) {
    let config = settings.read(cx).config().clone();
    editor.update(cx, |e, cx| {
        e.apply_config(&config, cx);
        let body = e
            .inspector()
            .and_then(|i| i.read(cx).body(Section::Background).cloned())
            .and_then(|b| b.downcast::<BackgroundBody>().ok());
        if let (Some(body), Some(chooser)) = (body, wallpaper) {
            body.update(cx, |b, _| b.set_chooser(chooser));
        }
        cx.observe(settings, |e, settings, cx| {
            let config = settings.read(cx).config().clone();
            e.apply_config(&config, cx);
        })
        .detach();
    });
}

pub(super) fn onboarding_input(w: &Wiring, cx: &App) -> OnboardingInput {
    let config = w.settings.read(cx).config();
    OnboardingInput {
        caps: w.caps,
        binds: w.binds.clone(),
        clipboard: Some(Rc::new(settings::SystemClipboard)),
        global_shortcuts: config.global_shortcuts.clone(),
        capture_shortcut: onboarding::capture_shortcut(&config.global_shortcuts),
        save_dir: es_store::paths::save_dir(config),
        autosave: config.general.auto_save,
    }
}

pub(super) fn open_practice(w: &Rc<Wiring>, cx: &mut App) {
    let file = w.paths.unstaged_dir().join("practice.png");
    let opened = es_store::atomic::write_atomic(&file, PRACTICE_IMAGE)
        .map_err(|e| Error::Protocol(format!("cannot write the practice image: {e}")))
        .and_then(|()| w.open_editor_for(&file, cx));
    if let Err(e) = opened {
        tracing::warn!(error = %e, "practice image");
        es_app::toast::show_toast(es_app::toast::Toast::error("toast-error-generic"), cx);
    }
}

fn open_onboarding(w: &Rc<Wiring>, cx: &mut App) -> Result<Rc<dyn WindowHandle>> {
    let input = onboarding_input(w, cx);
    let options = WindowOptions {
        window_bounds: Some(WindowBounds::centered(
            size(px(onboarding::WIDTH), px(onboarding::HEIGHT)),
            cx,
        )),
        app_id: Some(es_protocol::ONBOARDING_APP_ID.into()),
        ..Default::default()
    };
    let paths = w.paths.clone();
    let slot = Rc::new(RefCell::new(None::<Entity<Onboarding>>));
    let kept = slot.clone();
    let window = opened(cx.open_window(options, move |window, cx| {
        let view = cx.new(|cx| Onboarding::new(paths, input, window, cx));
        *kept.borrow_mut() = Some(view.clone());
        cx.new(|cx| Root::new(view, window, cx))
    }))?;
    if let Some(view) = slot.borrow_mut().take() {
        let events = w.clone();
        cx.subscribe(&view, move |_, event: &OnboardingEvent, cx| match event {
            OnboardingEvent::Closed => {
                let _ = window.update(cx, |_, window, _| window.remove_window());
            }
            OnboardingEvent::ChooseFolder => choose_save_folder(&events, cx),
            OnboardingEvent::SetAutosave(on) => {
                let on = *on;
                events
                    .settings
                    .update(cx, |s, cx| s.update(|c| c.general.auto_save = on, cx));
            }
            OnboardingEvent::SetShortcuts(keys) => {
                let keys = keys.clone();
                events
                    .settings
                    .update(cx, |s, cx| s.update(|c| c.global_shortcuts = keys, cx));
            }
            OnboardingEvent::CaptureArea => events.dispatch(Route::CaptureRegion, cx),
            OnboardingEvent::OpenPractice => open_practice(&events, cx),
        })
        .detach();
        let weak = view.downgrade();
        let wiring = w.clone();
        cx.observe(&w.settings, move |_, cx| {
            if let Some(view) = weak.upgrade() {
                let input = onboarding_input(&wiring, cx);
                view.update(cx, |v, cx| v.set_input(input, cx));
            }
        })
        .detach();
        let weak = view.downgrade();
        let history = w.history.clone();
        cx.subscribe(&w.history, move |_, event: &HistoryEvent, cx| {
            if let (HistoryEvent::Added(id), Some(view)) = (event, weak.upgrade())
                && let Some(entry) = history.read(cx).get(*id)
            {
                let file = entry.file.clone();
                view.update(cx, |v, cx| v.show_practice(file, cx));
            }
        })
        .detach();
    }
    Ok(handle(window))
}

struct BarHandle {
    window: AnyWindowHandle,
}

impl WindowHandle for BarHandle {
    fn is_open(&self, cx: &App) -> bool {
        self.window.is_open(cx)
    }
    fn focus(&self, cx: &mut App) {
        self.window.focus(cx)
    }
    fn close(&self, cx: &mut App) {
        WindowHandle::close(&self.window, cx)
    }
}

fn open_bar(w: &Rc<Wiring>, cx: &mut App) -> Result<Rc<dyn WindowHandle>> {
    let surfaces = w.surfaces.clone().ok_or(Error::Unsupported)?;
    let output = surfaces
        .test_output()
        .cloned()
        .or_else(|| surfaces.source().pointer_output())
        .or_else(|| surfaces.source().focused_output())
        .ok_or_else(|| Error::NotFound("an output for the capture bar".into()))?;
    let info = surfaces.output_info(&output)?;
    let bar = new_bar(w, cx);
    let surface = Rc::new(surfaces.open_surface(SurfaceRole::Bar, &output, BAR, cx)?);
    let place = {
        let (surface, bar, logical) = (surface.clone(), bar.clone(), info.logical);
        move |cx: &mut App| {
            let size = bar.read(cx).size(cx);
            surface.resize(size, cx);
            surface.set_position(placement::home(logical, size), cx);
            surface.set_input_region(
                &[es_geom::Rect::from_origin_size(es_geom::Point::ZERO, size)],
                cx,
            );
        }
    };
    surface.set_content(bar.clone().into(), cx);
    place(cx);
    let window = surface.window();
    let w = w.clone();
    cx.subscribe(&bar, move |_, event: &BarEvent, cx| match event {
        BarEvent::Resized => place(cx),
        BarEvent::Close => surface.close(cx),
        BarEvent::Route(route) => {
            surface.close(cx);
            w.dispatch(route.clone(), cx);
        }
        BarEvent::Record(request) => {
            surface.close(cx);
            let recordings = w.recordings.borrow().clone();
            let Some(recordings) = recordings else {
                return;
            };
            start_recording(&recordings, (**request).clone(), cx);
        }
        _ => {}
    })
    .detach();
    Ok(Rc::new(BarHandle { window }))
}
