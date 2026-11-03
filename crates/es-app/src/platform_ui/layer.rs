use super::SurfaceRole;
use crate::errors::{Error, Result};
use es_geom::{Point, Rect, Size};
use es_model::platform::OutputId;
use gpui::layer_shell::{Anchor, KeyboardInteractivity, Layer, LayerShellOptions};
use gpui::{Edges, Pixels, px};
use uuid::Uuid;

const SCALE_TOLERANCE: f64 = 0.01;

pub fn namespace(role: SurfaceRole) -> &'static str {
    match role {
        SurfaceRole::Overlay => "easyshot-overlay",
        SurfaceRole::Countdown => "easyshot-countdown",
        SurfaceRole::Toast => "easyshot-toast",
        SurfaceRole::Deck => "easyshot-deck",
        SurfaceRole::Pin => "easyshot-pin",
        SurfaceRole::Bar => "easyshot-bar",
        SurfaceRole::Highlight => "easyshot-rec-highlight",
        SurfaceRole::Scroll => "easyshot-scroll",
    }
}

/// `exclusive_keyboard` is honoured only for the overlay and the bar; the caller decides it with [`wants_exclusive`].
pub fn layer_options(role: SurfaceRole, exclusive_keyboard: bool) -> LayerShellOptions {
    use KeyboardInteractivity as K;
    let all = Anchor::TOP | Anchor::BOTTOM | Anchor::LEFT | Anchor::RIGHT;
    let placed = Anchor::TOP | Anchor::LEFT;
    // -1 keeps margins relative to the output edge rather than to other surfaces' reserved space.
    let free = Some(px(-1.));
    let grab = if exclusive_keyboard {
        K::Exclusive
    } else {
        K::OnDemand
    };
    let (layer, anchor, exclusive_zone, keyboard) = match role {
        SurfaceRole::Overlay => (Layer::Overlay, all, free, grab),
        SurfaceRole::Countdown => (Layer::Overlay, Anchor::empty(), None, K::None),
        SurfaceRole::Toast => (Layer::Top, Anchor::TOP, None, K::None),
        SurfaceRole::Deck => (
            Layer::Top,
            Anchor::BOTTOM | Anchor::RIGHT,
            free,
            K::OnDemand,
        ),
        SurfaceRole::Pin => (Layer::Top, placed, free, K::OnDemand),
        SurfaceRole::Bar => (Layer::Top, Anchor::BOTTOM, None, grab),
        SurfaceRole::Highlight | SurfaceRole::Scroll => (Layer::Overlay, placed, free, K::None),
    };
    LayerShellOptions {
        namespace: namespace(role).into(),
        layer,
        anchor,
        exclusive_zone,
        keyboard_interactivity: keyboard,
        ..Default::default()
    }
}

pub fn passes_input_through(role: SurfaceRole) -> bool {
    matches!(
        role,
        SurfaceRole::Countdown | SurfaceRole::Highlight | SurfaceRole::Scroll
    )
}

pub fn wants_exclusive(
    role: SurfaceRole,
    test_output: Option<&OutputId>,
    pointer_output: Option<&OutputId>,
    output: &OutputId,
) -> bool {
    test_output.is_none()
        && match role {
            SurfaceRole::Overlay => pointer_output == Some(output),
            // A compositor may hand an on-demand surface's focus back to the window under the pointer.
            SurfaceRole::Bar => true,
            _ => false,
        }
}

pub fn output_uuid(output: &OutputId) -> Uuid {
    Uuid::new_v5(&Uuid::NAMESPACE_DNS, output.0.as_bytes())
}

/// Never falls back to another display: the default one may be the user's real screen.
pub fn find_display<D>(
    displays: impl IntoIterator<Item = (D, Option<Uuid>)>,
    output: &OutputId,
) -> Result<D> {
    let want = output_uuid(output);
    displays
        .into_iter()
        .find(|(_, uuid)| *uuid == Some(want))
        .map(|(d, _)| d)
        .ok_or_else(|| Error::NotFound(format!("display for output {}", output.0)))
}

pub fn check_test_output(test_output: Option<&OutputId>, target: &OutputId) -> Result<()> {
    match test_output {
        Some(test) if test != target => Err(Error::Protocol(format!(
            "test mode: surface on {} refused, the test output is {}",
            target.0, test.0
        ))),
        _ => Ok(()),
    }
}

pub fn scale_ready(window_scale: f32, output_scale: f64) -> bool {
    (f64::from(window_scale) - output_scale).abs() < SCALE_TOLERANCE
}

/// Layer-shell margins in logical pixels that put the surface's top-left at `global`.
pub fn margins(anchor: Anchor, output: Rect, size: Size, global: Point) -> Edges<Pixels> {
    let left = global.x - output.x0;
    let top = global.y - output.y0;
    let right = output.width() - left - size.width;
    let bottom = output.height() - top - size.height;
    let edge = |a: Anchor, v: f64| {
        if anchor.contains(a) {
            px(v.round() as f32)
        } else {
            px(0.)
        }
    };
    Edges {
        top: edge(Anchor::TOP, top),
        right: edge(Anchor::RIGHT, right),
        bottom: edge(Anchor::BOTTOM, bottom),
        left: edge(Anchor::LEFT, left),
    }
}
