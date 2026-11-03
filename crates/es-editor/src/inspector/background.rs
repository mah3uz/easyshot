mod view;
pub mod wallpaper_cache;

use std::{path::PathBuf, rc::Rc, sync::Arc};

use es_model::{
    BackgroundSettings, BackgroundStyle,
    background::{Alignment, AspectRatio, BackgroundColor, ShadowStyle},
};
use es_render::beautify::gradients::{PRESETS, render_gradient};
use es_ui::components::{InspectorSlider, ValueFormat};
use gpui::{
    App, AppContext, Context, Entity, Hsla, PathPromptOptions, RenderImage, Rgba, Subscription,
    Task, Window, px,
};
use gpui_component::color_picker::{ColorPickerEvent, ColorPickerState};

use crate::canvas::underlay;
use crate::session::look::LookModel;
use crate::strings::fl;
use wallpaper_cache::{BUILT_IN, WallpaperCache};

const SHOWN_RECENT: usize = 4;
const RECENT_COLUMNS: u16 = 5;
const BUILT_IN_COLUMNS: u16 = 3;
const WALLPAPER_ASPECT: f32 = 1.35;
const GRADIENT_PIXELS: u32 = 64;
const GROUP_GAP: gpui::Pixels = px(16.);

/// Opens the system file chooser; tests hand the body a fake so no portal is ever reached.
pub trait WallpaperChooser {
    fn choose(&self, cx: &mut App) -> Task<Option<PathBuf>>;
}

pub struct PortalChooser;

impl WallpaperChooser for PortalChooser {
    fn choose(&self, cx: &mut App) -> Task<Option<PathBuf>> {
        let picked = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some(fl!("editor-bg-wallpaper-panel-title").into()),
        });
        cx.background_executor()
            .spawn(async move { picked.await.ok()?.ok()??.into_iter().next() })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Library {
    Color,
    Gradient,
    Wallpaper,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WallpaperTab {
    Recent,
    BuiltIn,
}

struct Controls {
    padding: Entity<InspectorSlider>,
    shadow: Entity<InspectorSlider>,
    corners: Entity<InspectorSlider>,
    picker: Entity<ColorPickerState>,
    _subscriptions: Vec<Subscription>,
}

/// The Background section: the fill library, then the layout of the card on it.
pub struct BackgroundBody {
    look: Entity<LookModel>,
    cache: Entity<WallpaperCache>,
    chooser: Rc<dyn WallpaperChooser>,
    library: Library,
    tab: WallpaperTab,
    recent: Vec<PathBuf>,
    gradients: Vec<Option<Arc<RenderImage>>>,
    controls: Option<Controls>,
    _subscriptions: Vec<Subscription>,
}

fn library_of(style: &BackgroundStyle) -> Option<Library> {
    match style {
        BackgroundStyle::None => None,
        BackgroundStyle::Solid(_) => Some(Library::Color),
        BackgroundStyle::Gradient(_) => Some(Library::Gradient),
        BackgroundStyle::Wallpaper { .. } => Some(Library::Wallpaper),
    }
}

impl BackgroundBody {
    pub fn new(look: Entity<LookModel>, cx: &mut Context<Self>) -> Self {
        let cache = cx.new(|_| WallpaperCache::default());
        let subscriptions = vec![
            cx.observe(&look, |this, _, cx| this.follow_look(cx)),
            cx.observe(&cache, |_, _, cx| cx.notify()),
        ];
        let gradients = PRESETS
            .iter()
            .map(|p| {
                render_gradient(&p.style(), GRADIENT_PIXELS, GRADIENT_PIXELS)
                    .and_then(|pixmap| underlay::render_image(&pixmap))
            })
            .collect();
        let mut this = Self {
            look,
            cache,
            chooser: Rc::new(PortalChooser),
            library: Library::Gradient,
            tab: WallpaperTab::Recent,
            recent: Vec::new(),
            gradients,
            controls: None,
            _subscriptions: subscriptions,
        };
        this.load_recent(cx);
        this.follow_style(cx);
        this
    }

    pub fn set_chooser(&mut self, chooser: Rc<dyn WallpaperChooser>) {
        self.chooser = chooser;
    }

    pub fn library(&self) -> Library {
        self.library
    }

    pub fn set_library(&mut self, library: Library, cx: &mut Context<Self>) {
        self.library = library;
        cx.notify();
    }

    pub fn wallpaper_tab(&self) -> WallpaperTab {
        self.tab
    }

    pub fn set_wallpaper_tab(&mut self, tab: WallpaperTab, cx: &mut Context<Self>) {
        self.tab = tab;
        cx.notify();
    }

    pub fn cache(&self) -> &Entity<WallpaperCache> {
        &self.cache
    }

    fn settings<'a>(&self, cx: &'a App) -> &'a BackgroundSettings {
        self.look.read(cx).settings()
    }

    fn set_style(&mut self, style: BackgroundStyle, cx: &mut Context<Self>) {
        self.look
            .update(cx, |look, cx| look.update(|s| s.style = style, cx));
    }

    pub fn pick_none(&mut self, cx: &mut Context<Self>) {
        self.set_style(BackgroundStyle::None, cx);
    }

    pub fn pick_color(&mut self, index: usize, cx: &mut Context<Self>) {
        if let Some(colour) = BackgroundColor::plain_presets().into_iter().nth(index) {
            self.set_style(BackgroundStyle::Solid(colour), cx);
        }
    }

    pub fn pick_custom(&mut self, colour: Hsla, cx: &mut Context<Self>) {
        let c = Rgba::from(colour);
        let custom = BackgroundColor::custom(f64::from(c.r), f64::from(c.g), f64::from(c.b));
        self.set_style(BackgroundStyle::Solid(custom), cx);
    }

    pub fn pick_gradient(&mut self, index: usize, cx: &mut Context<Self>) {
        if let Some(preset) = PRESETS.get(index) {
            self.set_style(BackgroundStyle::Gradient(preset.style()), cx);
        }
    }

    /// Written to the data directory first, since a document keeps a wallpaper by its path.
    pub fn pick_built_in(&mut self, index: usize, cx: &mut Context<Self>) {
        let Some(paths) = self.look.read(cx).paths().cloned() else {
            tracing::warn!("no data directory for the built-in wallpapers");
            return;
        };
        match BUILT_IN.get(index).map(|b| b.install(&paths)) {
            Some(Ok(path)) => self.set_style(BackgroundStyle::Wallpaper { path }, cx),
            Some(Err(error)) => tracing::warn!(%error, "cannot write the built-in wallpaper"),
            None => {}
        }
    }

    /// A picked wallpaper moves to the front of the recent list, so it stays among those shown.
    pub fn pick_recent(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        if let Some(paths) = self.look.read(cx).paths() {
            let saved = es_store::state::open(paths).and_then(|mut state| {
                state.value.push_recent_wallpaper(path.clone());
                state.save()?;
                Ok(state.value.recent_wallpapers)
            });
            match saved {
                Ok(recent) => self.recent = recent,
                Err(error) => tracing::warn!(%error, "cannot save the recent wallpapers"),
            }
        }
        self.cache.update(cx, |cache, _| cache.forget(&path));
        self.tab = WallpaperTab::Recent;
        self.set_style(BackgroundStyle::Wallpaper { path }, cx);
        cx.notify();
    }

    pub fn choose_wallpaper(&mut self, cx: &mut Context<Self>) -> Task<()> {
        let choosing = self.chooser.choose(cx);
        cx.spawn(async move |this, cx| {
            if let Some(path) = choosing.await {
                let _ = this.update(cx, |this, cx| this.pick_recent(path, cx));
            }
        })
    }

    pub fn set_shadow_style(&mut self, style: ShadowStyle, cx: &mut Context<Self>) {
        self.look
            .update(cx, |look, cx| look.update(|s| s.shadow_style = style, cx));
    }

    /// A fixed shape grows the stage around the card, which shows nothing without padding.
    pub fn set_aspect_ratio(&mut self, aspect: AspectRatio, cx: &mut Context<Self>) {
        if !self.shape_enabled(cx) {
            return;
        }
        self.look.update(cx, |look, cx| {
            look.update(
                |s| {
                    s.aspect_ratio = aspect;
                    if aspect != AspectRatio::Auto && s.padding == 0.0 {
                        s.padding = BackgroundSettings::default().padding;
                    }
                },
                cx,
            )
        });
    }

    /// Only a background, camera or bleeding blur puts the card on a stage that can take a shape.
    pub fn shape_enabled(&self, cx: &App) -> bool {
        self.settings(cx).uses_canvas_layout()
    }

    pub fn set_alignment(&mut self, alignment: Alignment, cx: &mut Context<Self>) {
        if self.alignment_enabled(cx) {
            self.look
                .update(cx, |look, cx| look.update(|s| s.alignment = alignment, cx));
        }
    }

    /// The camera frames the card itself, so alignment waits until it is reset.
    pub fn alignment_enabled(&self, cx: &App) -> bool {
        !self.settings(cx).camera.has_effect()
    }

    /// The recent wallpapers on show, newest first.
    pub fn shown_recent(&self) -> &[PathBuf] {
        &self.recent[..self.recent.len().min(SHOWN_RECENT)]
    }

    pub fn sliders(&self) -> Option<[Entity<InspectorSlider>; 3]> {
        let c = self.controls.as_ref()?;
        Some([c.padding.clone(), c.shadow.clone(), c.corners.clone()])
    }

    fn load_recent(&mut self, cx: &App) {
        let Some(paths) = self.look.read(cx).paths() else {
            return;
        };
        match es_store::state::open(paths) {
            Ok(state) => self.recent = state.value.recent_wallpapers,
            Err(error) => tracing::warn!(%error, "cannot read the recent wallpapers"),
        }
    }

    fn built_in_index(&self, cx: &App) -> Option<usize> {
        let look = self.look.read(cx);
        let BackgroundStyle::Wallpaper { path } = &look.settings().style else {
            return None;
        };
        let paths = look.paths()?;
        BUILT_IN.iter().position(|b| &b.path(paths) == path)
    }

    fn follow_style(&mut self, cx: &App) {
        if let Some(library) = library_of(&self.settings(cx).style) {
            self.library = library;
        }
        if library_of(&self.settings(cx).style) == Some(Library::Wallpaper) {
            self.tab = if self.built_in_index(cx).is_some() {
                WallpaperTab::BuiltIn
            } else {
                WallpaperTab::Recent
            };
        }
    }

    fn follow_look(&mut self, cx: &mut Context<Self>) {
        self.follow_style(cx);
        cx.notify();
    }

    /// The sliders and the colour picker need a window, which the inspector does not have when it builds its sections.
    pub fn ensure_controls(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.controls.is_some() {
            return;
        }
        let s = self.settings(cx).clone();
        let mut slider = |label: String, value: f64, max: f64| {
            cx.new(|cx| {
                InspectorSlider::new(label, value, 0.0..=max, ValueFormat::percent(), window, cx)
            })
        };
        let padding = slider(fl!("editor-bg-padding"), s.padding, 0.45);
        let shadow = slider(fl!("editor-bg-shadow"), s.shadow, 1.0);
        let corners = slider(fl!("editor-bg-corners"), s.corner_radius, 0.12);
        let picker = cx.new(|cx| ColorPickerState::new(window, cx));
        let look = self.look.clone();
        let subscriptions = vec![
            LookModel::bind_slider(&look, &padding, |s, v| s.padding = v, cx),
            LookModel::bind_slider(&look, &shadow, |s, v| s.shadow = v, cx),
            LookModel::bind_slider(&look, &corners, |s, v| s.corner_radius = v, cx),
            cx.subscribe_in(&picker, window, |this, _, event, _, cx| {
                if let ColorPickerEvent::Change(Some(colour)) = event {
                    this.pick_custom(*colour, cx);
                }
            }),
            cx.observe_in(&look, window, |this, _, window, cx| {
                this.sync_sliders(window, cx);
            }),
        ];
        self.controls = Some(Controls {
            padding,
            shadow,
            corners,
            picker,
            _subscriptions: subscriptions,
        });
        self.sync_sliders(window, cx);
    }

    fn sync_sliders(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(c) = &self.controls else {
            return;
        };
        let s = self.settings(cx).clone();
        let off = !s.requires_canvas_layout();
        for (slider, value) in [
            (&c.padding, s.padding),
            (&c.shadow, s.shadow),
            (&c.corners, s.corner_radius),
        ] {
            slider.update(cx, |slider, cx| {
                slider.set_value(value, window, cx);
                slider.set_disabled(off, window, cx);
            });
        }
    }
}
