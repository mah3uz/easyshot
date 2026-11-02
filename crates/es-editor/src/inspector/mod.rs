pub mod background;
pub mod blur;
pub mod border;
pub mod camera;
pub mod presets;
pub mod watermark;

use es_ui::{components::DisclosureSection, theme::tokens};
use gpui::{
    AnyView, AppContext, Context, Entity, InteractiveElement, IntoElement, ParentElement, Render,
    ScrollHandle, SharedString, StatefulInteractiveElement, Styled, Subscription, Window, div,
};

use crate::EditorCanvas;
use crate::session::look::LookModel;
pub use crate::session::look::Section;
use crate::strings::fl;
use blur::BlurBody;
use camera::CameraBody;
use presets::PresetsBar;

const SECTIONS: usize = Section::ALL.len();

fn index(section: Section) -> usize {
    match section {
        Section::Background => 0,
        Section::Camera => 1,
        Section::Blur => 2,
        Section::Border => 3,
        Section::Watermark => 4,
    }
}

fn title(section: Section) -> SharedString {
    match section {
        Section::Background => fl!("editor-section-background"),
        Section::Camera => fl!("editor-section-camera"),
        Section::Blur => fl!("editor-section-blur"),
        Section::Border => fl!("editor-section-border"),
        Section::Watermark => fl!("editor-section-watermark"),
    }
    .into()
}

fn clear_label(section: Section) -> Option<SharedString> {
    let label = match section {
        Section::Background => fl!("editor-clear-background"),
        Section::Camera => fl!("editor-clear-camera"),
        Section::Blur => fl!("editor-clear-blur"),
        Section::Border => fl!("editor-clear-border"),
        Section::Watermark => return None,
    };
    Some(label.into())
}

fn has_switch(section: Section) -> bool {
    matches!(section, Section::Blur | Section::Border)
}

/// The 300 px beautify column: the preset bar pinned on top, then the folding sections.
/// Each section's controls arrive through [`Inspector::set_body`].
pub struct Inspector {
    look: Entity<LookModel>,
    presets: Entity<PresetsBar>,
    expanded: [bool; SECTIONS],
    bodies: [Option<AnyView>; SECTIONS],
    scroll: ScrollHandle,
    _subscriptions: Vec<Subscription>,
}

impl Inspector {
    pub fn new(
        look: Entity<LookModel>,
        canvas: &Entity<EditorCanvas>,
        cx: &mut Context<Self>,
    ) -> Self {
        let mut expanded = [true, false, false, false, false];
        if let Some(paths) = look.read(cx).paths() {
            match es_store::state::open(paths) {
                Ok(state) => {
                    for section in Section::ALL {
                        if let Some(&open) = state.value.inspector_sections.get(section.key()) {
                            expanded[index(section)] = open;
                        }
                    }
                }
                Err(error) => tracing::warn!(%error, "cannot read the inspector state"),
            }
        }
        let presets = cx.new(|cx| PresetsBar::new(look.clone(), cx));
        let mut bodies: [Option<AnyView>; SECTIONS] = Default::default();
        bodies[index(Section::Background)] = Some(
            cx.new(|cx| background::BackgroundBody::new(look.clone(), cx))
                .into(),
        );
        bodies[index(Section::Camera)] =
            Some(cx.new(|cx| CameraBody::new(look.clone(), cx)).into());
        bodies[index(Section::Blur)] = Some(cx.new(|cx| BlurBody::new(look.clone(), cx)).into());
        let subscriptions = vec![
            LookModel::drive_canvas(&look, canvas, cx),
            cx.observe(&look, |_, _, cx| cx.notify()),
        ];
        bodies[index(Section::Border)] = Some(
            cx.new(|cx| border::BorderBody::new(look.clone(), cx))
                .into(),
        );
        bodies[index(Section::Watermark)] = Some(
            cx.new(|cx| watermark::WatermarkBody::new(look.clone(), cx))
                .into(),
        );
        Self {
            look,
            presets,
            expanded,
            bodies,
            scroll: ScrollHandle::new(),
            _subscriptions: subscriptions,
        }
    }

    pub fn look(&self) -> &Entity<LookModel> {
        &self.look
    }

    pub fn presets(&self) -> &Entity<PresetsBar> {
        &self.presets
    }

    /// Fills a section with its controls; the section keeps its header and accessories.
    pub fn set_body(&mut self, section: Section, body: AnyView, cx: &mut Context<Self>) {
        self.bodies[index(section)] = Some(body);
        cx.notify();
    }

    pub fn body(&self, section: Section) -> Option<&AnyView> {
        self.bodies[index(section)].as_ref()
    }

    pub fn is_expanded(&self, section: Section) -> bool {
        self.expanded[index(section)]
    }

    /// Remembered in `state.json` for every editor window.
    pub fn set_expanded(&mut self, section: Section, open: bool, cx: &mut Context<Self>) {
        if self.expanded[index(section)] == open {
            return;
        }
        self.expanded[index(section)] = open;
        if let Some(paths) = self.look.read(cx).paths() {
            let saved = es_store::state::open(paths).and_then(|mut state| {
                state
                    .value
                    .inspector_sections
                    .insert(section.key().to_owned(), open);
                state.save()
            });
            if let Err(error) = saved {
                tracing::warn!(%error, "cannot save the inspector state");
            }
        }
        cx.notify();
    }

    /// Switching blur or the border on also opens its section.
    pub fn set_enabled(&mut self, section: Section, on: bool, cx: &mut Context<Self>) {
        self.look
            .update(cx, |look, cx| look.set_enabled(section, on, cx));
        if on {
            self.set_expanded(section, true, cx);
        }
    }

    pub fn reset(&mut self, section: Section, cx: &mut Context<Self>) {
        self.look.update(cx, |look, cx| look.reset(section, cx));
    }

    fn section(&self, section: Section, cx: &mut Context<Self>) -> DisclosureSection {
        let look = self.look.read(cx);
        let this = cx.entity().downgrade();
        let on_toggle = this.clone();
        let mut disclosure =
            DisclosureSection::new(SharedString::from(section.key()), title(section))
                .expanded(self.is_expanded(section))
                .on_toggle(move |open, _, cx| {
                    let _ = on_toggle.update(cx, |this, cx| this.set_expanded(section, open, cx));
                });
        if has_switch(section) {
            let on_switch = this.clone();
            disclosure = disclosure.toggle(look.is_enabled(section), move |on, _, cx| {
                let _ = on_switch.update(cx, |this, cx| this.set_enabled(section, on, cx));
            });
        }
        if let Some(label) = clear_label(section).filter(|_| !look.is_default(section)) {
            disclosure = disclosure.reset(label, move |_, cx| {
                let _ = this.update(cx, |this, cx| this.reset(section, cx));
            });
        }
        disclosure.children(self.bodies[index(section)].clone())
    }
}

impl Render for Inspector {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = tokens(cx).clone();
        let sections: Vec<DisclosureSection> = Section::ALL
            .into_iter()
            .map(|s| self.section(s, cx))
            .collect();
        div()
            .id("inspector-content")
            .flex()
            .flex_col()
            .size_full()
            .child(
                div().flex_none().child(self.presets.clone()).child(
                    div()
                        .mx(t.metrics.inspector_padding)
                        .h(t.metrics.hairline)
                        .bg(t.separator.opacity(0.45)),
                ),
            )
            .child(
                div()
                    .id("inspector-sections")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .track_scroll(&self.scroll)
                    .children(sections),
            )
    }
}
