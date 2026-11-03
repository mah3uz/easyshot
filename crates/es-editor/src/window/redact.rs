use es_geom::{Rect, Size};
use es_model::{AnnoShape, AnnoShapeId, RedactionKind, RedactionProps, ShapeKind};
use es_ocr::redact::Kind;
use es_ui::{
    a11y::{Accessible, Role},
    components::EditorButton,
    tokens::{Tokens, Typeset},
};
use gpui::{
    AnyElement, Context, Entity, InteractiveElement, IntoElement, ParentElement, Styled, Task, div,
    prelude::FluentBuilder, px,
};
use gpui_component::{Sizable, spinner::Spinner};
use std::time::Duration;

use super::EditorWindow;
use crate::strings::fl;
use crate::toolbar::ToolRow;

const DENSITY: f64 = 0.8;
const REGION_FLASH: Duration = Duration::from_millis(700);
pub const TOAST_HOLD: Duration = Duration::from_secs(6);

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum State {
    #[default]
    Idle,
    Scanning,
    Added(usize),
}

#[derive(Default)]
pub(super) struct Tasks {
    scan: Option<Task<()>>,
    probe: Option<Task<()>>,
    reset: Option<Task<()>>,
    flash: Option<Task<()>>,
    toast: Option<Task<()>>,
}

/// What the last scan did; `added` is empty when it found nothing.
#[derive(Clone, Debug, PartialEq)]
pub struct Toast {
    pub text: String,
    pub added: Vec<AnnoShapeId>,
}

type Found = Vec<(Rect, Kind)>;

fn scan(base: &image::RgbaImage, options: &es_ocr::OcrOptions) -> Result<Found, es_ocr::Error> {
    let found = es_ocr::recognize(base, options)?;
    let size = Size::new(f64::from(base.width()), f64::from(base.height()));
    Ok(es_ocr::redact::find_kinds(&found.words, size))
}

fn kind_name(kind: Kind) -> String {
    match kind {
        Kind::Email => fl!("editor-redact-kind-email"),
        Kind::Link => fl!("editor-redact-kind-link"),
        Kind::IpAddress => fl!("editor-redact-kind-ip"),
        Kind::Phone => fl!("editor-redact-kind-phone"),
        Kind::Card => fl!("editor-redact-kind-card"),
        Kind::Key => fl!("editor-redact-kind-key"),
    }
}

pub fn toast_text(count: usize, kinds: &[Kind]) -> String {
    if count == 0 {
        return fl!("editor-redact-none-found");
    }
    let mut kinds = kinds.to_vec();
    kinds.sort();
    kinds.dedup();
    let names: Vec<String> = kinds.into_iter().map(kind_name).collect();
    fl!("editor-redact-hid", count = count, kinds = names.join(", "))
}

impl EditorWindow {
    pub fn set_ocr_options(&mut self, options: es_ocr::OcrOptions, cx: &mut Context<Self>) {
        self.ocr = options;
        self.probe_ocr(cx);
    }

    pub fn ocr_options(&self) -> &es_ocr::OcrOptions {
        &self.ocr
    }

    pub fn redact_state(&self) -> State {
        self.redact
    }

    pub fn redact_toast(&self) -> Option<&Toast> {
        self.redact_toast.as_ref()
    }

    /// Removes what the last scan added, in one undo step of its own.
    pub fn undo_redaction(&mut self, cx: &mut Context<Self>) {
        let Some(toast) = self.redact_toast.take() else {
            return;
        };
        self.redact_tasks.toast = None;
        if let Some(session) = self.session() {
            let doc = session.doc.clone();
            doc.update(cx, |d, cx| d.apply(cx, |e| e.delete_shapes(&toast.added)));
        }
        self.flash_regions(Vec::new(), cx);
        cx.notify();
    }

    fn flash_regions(&mut self, rects: Vec<Rect>, cx: &mut Context<Self>) {
        if let Some(canvas) = self.canvas() {
            let flashing = !rects.is_empty();
            canvas.update(cx, |c, cx| c.set_flash(rects, cx));
            self.redact_tasks.flash = flashing.then(|| {
                cx.spawn(async move |this, cx| {
                    cx.background_executor().timer(REGION_FLASH).await;
                    let _ = this.update(cx, |w, cx| w.flash_regions(Vec::new(), cx));
                })
            });
        }
    }

    fn row(&self) -> Option<Entity<ToolRow>> {
        self.tool_row.clone()?.downcast().ok()
    }

    pub(super) fn probe_ocr(&mut self, cx: &mut Context<Self>) {
        let path = self.ocr.tesseract_path.clone();
        self.redact_tasks.probe = Some(cx.spawn(async move |this, cx| {
            let probe = cx
                .background_executor()
                .spawn(async move { es_ocr::probe(path.as_deref()) })
                .await;
            let _ = this.update(cx, |w, cx| {
                if let Some(row) = w.row() {
                    row.update(cx, |r, cx| r.set_ocr_ready(probe.available, cx));
                }
            });
        }));
    }

    pub fn smart_redact(&mut self, kind: RedactionKind, cx: &mut Context<Self>) {
        if self.redact == State::Scanning {
            return;
        }
        let (Some(session), Some(row)) = (self.session(), self.row()) else {
            return;
        };
        let base = session.doc.read(cx).base().clone();
        let options = self.ocr.clone();
        self.set_redact(State::Scanning, &row, cx);
        self.redact_tasks.scan = Some(cx.spawn(async move |this, cx| {
            let found = cx
                .background_executor()
                .spawn(async move { scan(&base, &options) })
                .await;
            let _ = this.update(cx, |w, cx| w.finish_redact(kind, found, &row, cx));
        }));
    }

    fn finish_redact(
        &mut self,
        kind: RedactionKind,
        found: Result<Found, es_ocr::Error>,
        row: &Entity<ToolRow>,
        cx: &mut Context<Self>,
    ) {
        let rects = match found {
            Ok(rects) => rects,
            Err(error) => {
                tracing::warn!(%error, "smart redaction scan failed");
                if matches!(error, es_ocr::Error::NotInstalled) {
                    row.update(cx, |r, cx| r.set_ocr_ready(false, cx));
                    self.error = Some(fl!("editor-redact-unavailable").into());
                } else {
                    self.error =
                        Some(fl!("editor-redact-failed", error = error.to_string()).into());
                }
                self.set_redact(State::Idle, row, cx);
                return;
            }
        };
        tracing::debug!(regions = rects.len(), "smart redaction found regions");
        let kinds: Vec<Kind> = rects.iter().map(|(_, k)| *k).collect();
        let regions: Vec<Rect> = rects.iter().map(|(r, _)| *r).collect();
        let shapes: Vec<AnnoShape> = regions
            .iter()
            .map(|r| {
                AnnoShape::new(
                    r.x0,
                    r.y0,
                    ShapeKind::Redaction(RedactionProps {
                        kind,
                        w: r.width(),
                        h: r.height(),
                        density: DENSITY,
                    }),
                )
            })
            .collect();
        let added: Vec<AnnoShapeId> = shapes.iter().map(|s| s.id).collect();
        if let Some(session) = self.session() {
            let doc = session.doc.clone();
            doc.update(cx, |d, cx| d.apply(cx, |e| e.add_shapes(shapes)));
        }
        self.redact_toast = Some(Toast {
            text: toast_text(added.len(), &kinds),
            added,
        });
        self.redact_tasks.toast = Some(cx.spawn(async move |this, cx| {
            cx.background_executor().timer(TOAST_HOLD).await;
            let _ = this.update(cx, |w, cx| {
                w.redact_toast = None;
                cx.notify();
            });
        }));
        self.flash_regions(regions, cx);
        self.set_redact(State::Added(rects.len()), row, cx);
        self.redact_tasks.reset = Some(cx.spawn(async move |this, cx| {
            cx.background_executor().timer(super::FLASH).await;
            let _ = this.update(cx, |w, cx| {
                if matches!(w.redact, State::Added(_)) {
                    w.redact = State::Idle;
                    cx.notify();
                }
            });
        }));
    }

    fn set_redact(&mut self, state: State, row: &Entity<ToolRow>, cx: &mut Context<Self>) {
        self.redact = state;
        row.update(cx, |r, cx| r.set_redact_busy(state == State::Scanning, cx));
        cx.notify();
    }

    pub(crate) fn redact_status(&self, t: &Tokens) -> Option<AnyElement> {
        if self.redact != State::Scanning {
            return None;
        }
        let text = fl!("editor-redact-scanning");
        let scanning = true;
        Some(
            div()
                .id("redact-status")
                .flex()
                .items_center()
                .gap(px(6.))
                .text_color(t.text_secondary)
                .typeset(t.type_scale.label)
                .accessible(
                    if scanning {
                        Role::ProgressIndicator
                    } else {
                        Role::Status
                    },
                    text.clone(),
                )
                .when(scanning, |d| {
                    d.child(Spinner::new().small().color(t.text_secondary))
                })
                .child(text)
                .into_any_element(),
        )
    }
}

impl EditorWindow {
    pub(crate) fn redact_toast_view(
        &self,
        t: &Tokens,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let toast = self.redact_toast.as_ref()?;
        let undoable = !toast.added.is_empty();
        Some(
            div()
                .id("redact-toast-row")
                .absolute()
                .left_0()
                .right_0()
                .bottom(px(52.))
                .flex()
                .justify_center()
                .child(
                    div()
                        .id("redact-toast")
                        .flex()
                        .items_center()
                        .gap(px(12.))
                        .py(px(8.))
                        .px(px(14.))
                        .rounded(t.metrics.popover_radius)
                        .bg(t.dropdown)
                        .border(t.metrics.hairline)
                        .border_color(t.border)
                        .typeset(t.type_scale.label)
                        .text_color(t.text)
                        .accessible(Role::Status, toast.text.clone())
                        .child(toast.text.clone())
                        .when(undoable, |d| {
                            d.child(
                                EditorButton::new("redact-undo")
                                    .label(fl!("editor-redact-undo"))
                                    .on_click(cx.listener(|w, _, _, cx| w.undo_redaction(cx))),
                            )
                        }),
                )
                .into_any_element(),
        )
    }
}
