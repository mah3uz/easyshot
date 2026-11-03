use super::underlay::Morph;
use super::{EditorCanvas, effects};
use crate::session::viewport::ViewMath;
use es_geom::{Point, Rect};
use es_model::BackgroundSettings;
use es_render::beautify::{Layout, layout};
use es_ui::motion::{CROP_COMMIT, Clocked, Transition};
use gpui::{Context, Window};

enum Shift {
    /// The new image starts where the applied crop rectangle sat on screen.
    View { from: Rect },
    Look {
        from: Box<BackgroundSettings>,
        layout: Layout,
    },
}

pub(super) struct CanvasMotion {
    clocked: Clocked,
    shift: Shift,
}

/// What a frame draws: the view, and the look part of the way along when it is changing.
pub(super) struct Shown {
    pub math: ViewMath,
    pub morph: Option<Morph>,
}

impl EditorCanvas {
    /// `from` is the crop rectangle in the canvas element, before the crop was applied.
    pub(super) fn crop_committed(&mut self, from: Rect, cx: &mut Context<Self>) {
        self.motion = Clocked::start(CROP_COMMIT, cx).map(|clocked| CanvasMotion {
            clocked,
            shift: Shift::View { from },
        });
        cx.notify();
    }

    /// The camera and scene blur are drawn as one composite of the final look, so they change at once.
    pub(super) fn look_changing(
        &mut self,
        to: &BackgroundSettings,
        transition: Option<Transition>,
        cx: &mut Context<Self>,
    ) {
        let from = self.background.clone();
        let composite = effects::active(&from) || effects::active(to);
        self.motion = transition
            .filter(|_| !composite)
            .and_then(|t| Clocked::start(t, cx))
            .map(|clocked| {
                let size = self.doc.read(cx).image_size();
                CanvasMotion {
                    clocked,
                    shift: Shift::Look {
                        layout: layout(size.width, size.height, &from).pixel_aligned(),
                        from: Box::new(from),
                    },
                }
            });
    }

    pub(super) fn shown(
        &mut self,
        math: ViewMath,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Shown {
        let Some(motion) = &self.motion else {
            return Shown { math, morph: None };
        };
        let t = f64::from(motion.clocked.progress(window, cx));
        if t >= 1.0 {
            self.motion = None;
            return Shown { math, morph: None };
        }
        match &motion.shift {
            Shift::View { from } => Shown {
                math: from_rect(math, *from, t),
                morph: None,
            },
            Shift::Look { from, layout } => Shown {
                math: ViewMath {
                    layout: layout.lerp(&math.layout, t),
                    ..math
                },
                morph: Some(Morph {
                    from: (**from).clone(),
                    t,
                }),
            },
        }
    }
}

/// The view `t` of the way from the image filling `from` to `to`.
fn from_rect(to: ViewMath, from: Rect, t: f64) -> ViewMath {
    let image = to.layout.image;
    let start = from.width() / image.width().max(1.0);
    let canvas = (to.layout.canvas_width, to.layout.canvas_height);
    let pan_at = |s: f64| {
        Point::new(
            from.x0 - image.x0 * s - (to.container.width - canvas.0 * s) / 2.0,
            from.y0 - image.y0 * s - (to.container.height - canvas.1 * s) / 2.0,
        )
    };
    let (s0, p0) = (start, pan_at(start));
    let (s1, p1) = (to.scale(), to.pan);
    let f = |a: f64, b: f64| a + (b - a) * t;
    ViewMath {
        fit: false,
        manual_scale: f(s0, s1),
        pan: Point::new(f(p0.x, p1.x), f(p0.y, p1.y)),
        ..to
    }
}
