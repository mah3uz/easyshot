use super::EditorCanvas;
use super::prepare::Prepared;
use gpui::{
    App, Bounds, ContentMask, Corners, CursorStyle, DispatchPhase, Element, ElementId, Entity,
    GlobalElementId, Hitbox, HitboxBehavior, InspectorElementId, IntoElement, LayoutId,
    MouseMoveEvent, MouseUpEvent, Pixels, Style, Window, relative,
};

pub struct CanvasElement {
    view: Entity<EditorCanvas>,
}

impl CanvasElement {
    pub fn new(view: Entity<EditorCanvas>) -> Self {
        Self { view }
    }
}

impl IntoElement for CanvasElement {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for CanvasElement {
    type RequestLayoutState = ();
    type PrepaintState = (Hitbox, Option<Prepared>);

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static std::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, ()) {
        let mut style = Style::default();
        style.size.width = relative(1.).into();
        style.size.height = relative(1.).into();
        (window.request_layout(style, [], cx), ())
    }

    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState {
        let hitbox = window.insert_hitbox(bounds, HitboxBehavior::Normal);
        let prepared = self
            .view
            .update(cx, |canvas, cx| canvas.prepare(bounds, window, cx));
        (hitbox, Some(prepared))
    }

    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut (),
        (hitbox, prepared): &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        let Some(frame) = prepared.take() else {
            return;
        };
        for image in frame.retired {
            if let Err(error) = window.drop_image(image) {
                tracing::debug!(%error, "dropping a replaced canvas image");
            }
        }
        let chrome = self.view.read(cx).chrome(bounds, window.scale_factor(), cx);
        let crop = self
            .view
            .update(cx, |c, cx| c.crop_overlay(bounds, window, cx));
        window.with_content_mask(Some(ContentMask { bounds }), |window| {
            if let Some((image, cells)) = frame.grid {
                for at in cells {
                    paint_image(window, bounds, at, Corners::default(), image.clone());
                }
            }
            if let Some((at, image)) = frame.underlay {
                paint_image(window, bounds, at, Corners::default(), image);
            }
            for (at, corners, image) in frame.tiles {
                paint_image(window, bounds, at, corners, image);
            }
            let flat = frame.effects.is_none();
            if let Some(painting) = frame.effects {
                if let Some((at, image)) = painting.under {
                    paint_image(window, bounds, at, Corners::default(), image);
                }
                window.with_element_opacity(Some(painting.opacity), |window| {
                    paint_image(
                        window,
                        bounds,
                        painting.at,
                        Corners::default(),
                        painting.image,
                    );
                });
            }
            let view = self.view.clone();
            let marker = view.update(cx, |c, _| {
                if flat {
                    c.highlighter_painter.take()
                } else {
                    None
                }
            });
            if let Some(marker) = marker {
                marker(&frame.highlighters, &frame.viewport, bounds, window, cx);
                view.update(cx, |c, _| {
                    c.highlighter_painter.get_or_insert(marker);
                });
            }
            let painter = view.update(cx, |c, _| {
                if flat {
                    c.redaction_painter.take()
                } else {
                    None
                }
            });
            // An empty list still reaches the painter, so it can release the patches of deleted redactions.
            if let Some(painter) = painter {
                painter(&frame.redactions, &frame.viewport, bounds, window, cx);
                view.update(cx, |c, _| {
                    c.redaction_painter.get_or_insert(painter);
                });
            }
            if !frame.spotlight.is_empty() {
                let clip = ContentMask {
                    bounds: frame.image.intersect(&bounds),
                };
                window.with_content_mask(Some(clip), |window| {
                    for p in frame.spotlight {
                        window.paint_path(p.path, p.color);
                    }
                });
            }
            let mut shadows = frame.shadows.into_iter().peekable();
            for (i, p) in frame.shapes.into_iter().enumerate() {
                while let Some((_, at, image)) = shadows.next_if(|s| s.0 == i) {
                    paint_image(window, bounds, at, Corners::default(), image);
                }
                window.paint_path(p.path, p.color);
            }
            if let Some((at, image)) = frame.watermark {
                paint_image(window, bounds, at, Corners::default(), image);
            }
            for p in chrome.into_iter().chain(crop) {
                window.paint_path(p.path, p.color);
            }
        });
        super::text_edit::register_input(&self.view, bounds, window, cx);
        self.register_pan(hitbox, window, cx);
    }
}

fn paint_image(
    window: &mut Window,
    bounds: Bounds<Pixels>,
    at: Bounds<Pixels>,
    corners: Corners<Pixels>,
    image: std::sync::Arc<gpui::RenderImage>,
) {
    if let Err(error) = window.paint_image(bounds, at, corners, image, 0, false) {
        tracing::warn!(%error, "a canvas image could not be painted");
    }
}

impl CanvasElement {
    fn register_pan(&self, hitbox: &Hitbox, window: &mut Window, cx: &mut App) {
        let focused = self.view.read(cx).focus.is_focused(window);
        let (panning, space) = self.view.update(cx, |c, _| {
            if !focused && c.space_held {
                c.space_held = false;
                c.pan_from = None;
            }
            (c.pan_from.is_some(), c.space_held)
        });
        if panning {
            self.view
                .update(cx, |c, _| c.cursor = CursorStyle::ClosedHand);
            window.set_cursor_style(CursorStyle::ClosedHand, hitbox);
            let view = self.view.clone();
            window.on_mouse_event(move |e: &MouseMoveEvent, phase, _, cx| {
                if phase == DispatchPhase::Capture {
                    view.update(cx, |c, cx| {
                        if e.pressed_button.is_some() {
                            c.drag_pan(e.position, cx);
                        } else {
                            c.end_pan(cx);
                        }
                    });
                }
            });
            let view = self.view.clone();
            window.on_mouse_event(move |_: &MouseUpEvent, phase, _, cx| {
                if phase == DispatchPhase::Capture {
                    view.update(cx, |c, cx| c.end_pan(cx));
                }
            });
        } else if space {
            super::input::register_space(&self.view, hitbox, window, cx);
        } else if self.view.read(cx).is_cropping(cx) {
            super::crop_overlay::register(&self.view, hitbox, window, cx);
        } else {
            super::input::register(&self.view, hitbox, window, cx);
        }
    }
}
