use super::doc::{DocEvent, DocModel};
use es_model::StylePreset;
use es_store::Paths;
use gpui::{AppContext, Context, Entity, Subscription, Task};
use std::time::Duration;

pub const DELAY: Duration = Duration::from_millis(400);

/// The saved style, or the defaults when the file is missing, damaged or written by a newer easyshot.
pub fn saved(paths: Option<&Paths>) -> StylePreset {
    let Some(paths) = paths else {
        return StylePreset::default();
    };
    match es_store::presets::open_style(paths) {
        Ok(file) if file.newer_version().is_none() => file.value,
        Ok(_) => StylePreset::default(),
        Err(error) => {
            tracing::warn!(%error, "annotation style unreadable; using the defaults");
            StylePreset::default()
        }
    }
}

/// Re-reads the file first, so a newer or damaged one is left alone.
fn write(paths: &Paths, preset: StylePreset) {
    match es_store::presets::open_style(paths) {
        Ok(mut file) if file.newer_version().is_none() => {
            file.value = preset;
            if let Err(error) = file.save() {
                tracing::warn!(%error, "annotation style not saved");
            }
        }
        Ok(_) => {}
        Err(error) => tracing::warn!(%error, "annotation style not saved"),
    }
}

/// Saves the editor's style a moment after it last changed.
pub struct StyleStore {
    doc: Entity<DocModel>,
    paths: Option<Paths>,
    unsaved: Option<StylePreset>,
    timer: Option<Task<()>>,
    _on_tool: Subscription,
}

impl StyleStore {
    pub fn new(doc: Entity<DocModel>, paths: Option<Paths>, cx: &mut Context<Self>) -> Self {
        let on_tool = cx.subscribe(&doc, |this, _, event: &DocEvent, cx| {
            if *event == DocEvent::ToolChanged {
                this.changed(cx);
            }
        });
        Self {
            doc,
            paths,
            unsaved: None,
            timer: None,
            _on_tool: on_tool,
        }
    }

    pub fn changed(&mut self, cx: &mut Context<Self>) {
        if self.paths.is_none() {
            return;
        }
        self.unsaved = Some(self.doc.read(cx).editor().preset());
        self.timer = Some(cx.spawn(async move |this, cx| {
            cx.background_executor().timer(DELAY).await;
            let _ = this.update(cx, |this, cx| this.flush(cx));
        }));
    }

    fn flush(&mut self, cx: &mut Context<Self>) {
        self.timer = None;
        if let (Some(paths), Some(preset)) = (self.paths.clone(), self.unsaved.take()) {
            cx.background_spawn(async move { write(&paths, preset) })
                .detach();
        }
    }
}

impl Drop for StyleStore {
    fn drop(&mut self) {
        if let (Some(paths), Some(preset)) = (&self.paths, self.unsaved.take()) {
            write(paths, preset);
        }
    }
}
