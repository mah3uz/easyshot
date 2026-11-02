use es_model::{
    AnnotationDocument, BackgroundPreset, BackgroundSettings, BackgroundStyle, Config,
    PresetLibrary, resolve_new_capture_look,
};
use es_store::{Paths, presets::BackgroundPresetFile};
use es_ui::components::{InspectorSlider, SliderEvent};
use gpui::{App, Context, Entity, EventEmitter, Subscription};
use uuid::Uuid;

use crate::{EditorCanvas, session::doc::DocModel};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Section {
    Background,
    Camera,
    Blur,
    Border,
    Watermark,
}

impl Section {
    pub const ALL: [Section; 5] = [
        Section::Background,
        Section::Camera,
        Section::Blur,
        Section::Border,
        Section::Watermark,
    ];

    /// The key in `state.json` `inspector_sections`.
    pub fn key(self) -> &'static str {
        match self {
            Section::Background => "background",
            Section::Camera => "camera",
            Section::Blur => "blur",
            Section::Border => "border",
            Section::Watermark => "watermark",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LookEvent {
    Changed,
    LiveAdjust(bool),
    PresetsChanged,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum PresetError {
    #[error("a preset with this name already exists")]
    Duplicate,
    #[error("the selected wallpaper cannot be found")]
    WallpaperMissing,
}

/// The beautify look of one image. Changes are not undoable; they reach the document so a save
/// keeps them, and the canvas through [`LookEvent`].
pub struct LookModel {
    settings: BackgroundSettings,
    applied: Option<Uuid>,
    library: Option<BackgroundPresetFile>,
    paths: Option<Paths>,
    doc: Entity<DocModel>,
}

impl EventEmitter<LookEvent> for LookModel {}

pub fn wallpaper_missing(settings: &BackgroundSettings) -> bool {
    matches!(&settings.style, BackgroundStyle::Wallpaper { path } if !path.exists())
}

fn open_library(paths: Option<&Paths>) -> Option<BackgroundPresetFile> {
    match es_store::presets::open_backgrounds(paths?) {
        Ok(file) => Some(file),
        Err(error) => {
            tracing::warn!(%error, "cannot read the background presets");
            None
        }
    }
}

/// A document with nothing to render was never beautified, so it opens with the look for new captures.
pub fn opening_look(
    document: &mut AnnotationDocument,
    paths: Option<&Paths>,
    config: Option<&Config>,
) {
    if document.has_content() {
        return;
    }
    if let Some(config) = config {
        let library = open_library(paths).map(|f| f.value).unwrap_or_default();
        document.background = resolve_new_capture_look(config, &library);
    }
}

impl LookModel {
    pub fn new(doc: Entity<DocModel>, paths: Option<Paths>, cx: &App) -> Self {
        let settings = doc.read(cx).background().clone();
        let library = open_library(paths.as_ref());
        let applied = library.as_ref().and_then(|file| {
            file.value
                .presets
                .iter()
                .find(|p| p.background.same_look_as(&settings))
                .map(|p| p.id)
        });
        Self {
            settings,
            applied,
            library,
            paths,
            doc,
        }
    }

    pub fn settings(&self) -> &BackgroundSettings {
        &self.settings
    }

    pub fn paths(&self) -> Option<&Paths> {
        self.paths.as_ref()
    }

    pub fn update(&mut self, f: impl FnOnce(&mut BackgroundSettings), cx: &mut Context<Self>) {
        let mut next = self.settings.clone();
        f(&mut next);
        self.set(next, cx);
    }

    pub fn set(&mut self, settings: BackgroundSettings, cx: &mut Context<Self>) {
        if settings == self.settings {
            return;
        }
        self.settings = settings.clone();
        self.doc.update(cx, |d, cx| d.set_background(settings, cx));
        cx.emit(LookEvent::Changed);
        cx.notify();
    }

    /// Brackets a slider or pad drag so the canvas draws a fast approximation meanwhile.
    pub fn begin_live_adjust(&mut self, cx: &mut Context<Self>) {
        cx.emit(LookEvent::LiveAdjust(true));
    }

    pub fn end_live_adjust(&mut self, cx: &mut Context<Self>) {
        cx.emit(LookEvent::LiveAdjust(false));
    }

    pub fn is_default(&self, section: Section) -> bool {
        let s = &self.settings;
        let d = BackgroundSettings::default();
        match section {
            Section::Background => s.style == BackgroundStyle::None,
            Section::Camera => s.camera == d.camera,
            Section::Blur => s.progressive_blur == d.progressive_blur,
            Section::Border => s.border == d.border,
            Section::Watermark => s.watermark == d.watermark,
        }
    }

    pub fn reset(&mut self, section: Section, cx: &mut Context<Self>) {
        let d = BackgroundSettings::default();
        self.update(
            |s| match section {
                Section::Background => s.style = BackgroundStyle::None,
                Section::Camera => s.camera = d.camera,
                Section::Blur => s.progressive_blur = d.progressive_blur,
                Section::Border => s.border = d.border,
                Section::Watermark => s.watermark = d.watermark,
            },
            cx,
        );
    }

    /// Only progressive blur and the border have a switch of their own.
    pub fn is_enabled(&self, section: Section) -> bool {
        match section {
            Section::Blur => self.settings.progressive_blur.is_enabled,
            Section::Border => self.settings.border.is_enabled,
            _ => true,
        }
    }

    pub fn set_enabled(&mut self, section: Section, on: bool, cx: &mut Context<Self>) {
        self.update(
            |s| match section {
                Section::Blur => s.progressive_blur.is_enabled = on,
                Section::Border => s.border.is_enabled = on,
                _ => {}
            },
            cx,
        );
    }

    /// Wires a slider so a drag is one live adjustment and every value reaches the look.
    pub fn bind_slider(
        look: &Entity<Self>,
        slider: &Entity<InspectorSlider>,
        apply: impl Fn(&mut BackgroundSettings, f64) + 'static,
        cx: &mut App,
    ) -> Subscription {
        let look = look.downgrade();
        cx.subscribe(slider, move |_, event: &SliderEvent, cx| {
            let _ = look.update(cx, |look, cx| match *event {
                SliderEvent::DragStarted => look.begin_live_adjust(cx),
                SliderEvent::Changed(value) => look.update(|s| apply(s, value), cx),
                SliderEvent::DragEnded => look.end_live_adjust(cx),
            });
        })
    }

    /// Sends every change and live adjustment on to the canvas.
    pub fn drive_canvas(
        look: &Entity<Self>,
        canvas: &Entity<EditorCanvas>,
        cx: &mut App,
    ) -> Subscription {
        let canvas = canvas.downgrade();
        cx.subscribe(look, move |look, event: &LookEvent, cx| {
            let _ = canvas.update(cx, |canvas, cx| match *event {
                LookEvent::Changed => canvas.set_background(look.read(cx).settings.clone(), cx),
                LookEvent::LiveAdjust(true) => canvas.begin_live_adjust(cx),
                LookEvent::LiveAdjust(false) => canvas.end_live_adjust(cx),
                LookEvent::PresetsChanged => {}
            });
        })
    }

    pub fn presets(&self) -> &[BackgroundPreset] {
        self.library
            .as_ref()
            .map_or(&[], |file| file.value.presets.as_slice())
    }

    pub fn applied(&self) -> Option<Uuid> {
        self.applied
    }

    pub fn applied_preset(&self) -> Option<&BackgroundPreset> {
        let id = self.applied?;
        self.presets().iter().find(|p| p.id == id)
    }

    /// The applied preset no longer matches the canvas.
    pub fn modified(&self) -> bool {
        self.applied_preset()
            .is_some_and(|p| !p.background.same_look_as(&self.settings))
    }

    /// The preset new screenshots open with.
    pub fn default_preset(&self) -> Option<Uuid> {
        self.library.as_ref()?.value.active_preset_id
    }

    pub fn apply_preset(&mut self, id: Option<Uuid>, cx: &mut Context<Self>) {
        let look = id.and_then(|id| self.presets().iter().find(|p| p.id == id).cloned());
        self.applied = look.as_ref().map(|p| p.id);
        if let Some(preset) = look {
            self.set(preset.background, cx);
        }
        cx.emit(LookEvent::PresetsChanged);
        cx.notify();
    }

    /// Saves the current look under `name`; it becomes the default for new screenshots.
    pub fn add_preset(&mut self, name: &str, cx: &mut Context<Self>) -> Result<Uuid, PresetError> {
        if wallpaper_missing(&self.settings) {
            return Err(PresetError::WallpaperMissing);
        }
        let settings = self.settings.clone();
        let id = self.edit_library(cx, |lib| {
            let wanted = PresetLibrary::sanitize_name(name);
            if lib.unique_name(&wanted, None) != wanted {
                return Err(PresetError::Duplicate);
            }
            Ok(lib.add(&wanted, settings))
        })?;
        self.applied = Some(id);
        Ok(id)
    }

    /// The canvas keeps its look.
    pub fn delete_preset(&mut self, id: Uuid, cx: &mut Context<Self>) {
        let _ = self.edit_library(cx, |lib| Ok(lib.remove(id)));
        if self.applied == Some(id) {
            self.applied = None;
        }
    }

    pub fn set_default_preset(&mut self, id: Option<Uuid>, cx: &mut Context<Self>) {
        let _ = self.edit_library(cx, |lib| {
            lib.active_preset_id = id.filter(|id| lib.get(*id).is_some());
            Ok(())
        });
    }

    fn edit_library<T>(
        &mut self,
        cx: &mut Context<Self>,
        f: impl FnOnce(&mut PresetLibrary) -> Result<T, PresetError>,
    ) -> Result<T, PresetError> {
        let paths = self.paths.clone();
        let file = self.library.get_or_insert_with(|| {
            let path = paths.map(|p| p.bg_presets_file()).unwrap_or_default();
            BackgroundPresetFile::fresh(&path, es_model::FormatKind::BgPresets)
        });
        let out = f(&mut file.value)?;
        if self.paths.is_some()
            && let Err(error) = file.save()
        {
            tracing::warn!(%error, "cannot save the background presets");
        }
        cx.emit(LookEvent::PresetsChanged);
        cx.notify();
        Ok(out)
    }
}
