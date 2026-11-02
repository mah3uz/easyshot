use es_geom::{Rect, Size};
use es_model::{AnnoShape, RedactionKind, RedactionProps, ShapeKind};
use es_ui::{
    a11y::{Accessible, Role},
    tokens::{Tokens, Typeset},
};
use gpui::{
    AnyElement, Context, Entity, InteractiveElement, IntoElement, ParentElement, Styled, Task, div,
    prelude::FluentBuilder, px,
};
use gpui_component::{Sizable, spinner::Spinner};

use super::{EditorWindow, FLASH};
use crate::strings::fl;
use crate::toolbar::ToolRow;

const DENSITY: f64 = 0.8;

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
}

fn scan(base: &image::RgbaImage, options: &es_ocr::OcrOptions) -> Result<Vec<Rect>, es_ocr::Error> {
    let found = es_ocr::recognize(base, options)?;
    let size = Size::new(f64::from(base.width()), f64::from(base.height()));
    Ok(es_ocr::redact::find(&found.words, size))
}

impl EditorWindow {
    pub fn set_ocr_options(&mut self, options: es_ocr::OcrOptions, cx: &mut Context<Self>) {
        self.ocr = options;
        self.probe_ocr(cx);
    }

    pub fn redact_state(&self) -> State {
        self.redact
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
        found: Result<Vec<Rect>, es_ocr::Error>,
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
        let shapes: Vec<AnnoShape> = rects
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
        if let Some(session) = self.session() {
            let doc = session.doc.clone();
            doc.update(cx, |d, cx| d.apply(cx, |e| e.add_shapes(shapes)));
        }
        self.set_redact(State::Added(rects.len()), row, cx);
        self.redact_tasks.reset = Some(cx.spawn(async move |this, cx| {
            cx.background_executor().timer(FLASH).await;
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
        let text = match self.redact {
            State::Idle => return None,
            State::Scanning => fl!("editor-redact-scanning"),
            State::Added(0) => fl!("editor-redact-none-found"),
            State::Added(count) => fl!("editor-redact-added", count = count),
        };
        let scanning = self.redact == State::Scanning;
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
