use crate::tray::IconPixmap;
use resvg::{tiny_skia, usvg};

const SVG: &str = include_str!("../../../assets/icons/app/easyshot.svg");
pub const SIZES: [u32; 4] = [16, 22, 32, 44];
const RED: [u8; 3] = [0xe5, 0x48, 0x4d];
const AMBER: [u8; 3] = [0xe5, 0xa5, 0x0a];

fn render(size: u32) -> Option<IconPixmap> {
    let tree = usvg::Tree::from_str(SVG, &usvg::Options::default()).ok()?;
    let mut pixmap = tiny_skia::Pixmap::new(size, size)?;
    let scale = size as f32 / tree.size().width();
    resvg::render(
        &tree,
        tiny_skia::Transform::from_scale(scale, scale),
        &mut pixmap.as_mut(),
    );
    let mut argb = Vec::with_capacity(pixmap.data().len());
    for px in pixmap.pixels() {
        let c = px.demultiply();
        argb.extend_from_slice(&[c.alpha(), c.red(), c.green(), c.blue()]);
    }
    Some(IconPixmap {
        width: size as i32,
        height: size as i32,
        argb,
    })
}

/// The full-colour app icon at the sizes the status notifier host picks from, as straight-alpha ARGB32.
pub fn tray_icons() -> Vec<IconPixmap> {
    SIZES.iter().filter_map(|s| render(*s)).collect()
}

/// Draws the recording dot, red or amber when paused, in the lower right corner.
pub fn with_dot(icon: &IconPixmap, paused: bool) -> IconPixmap {
    let mut out = icon.clone();
    let side = icon.width.min(icon.height) as f32;
    let radius = side * 0.24;
    let (cx, cy) = (side - radius - side * 0.02, side - radius - side * 0.02);
    let rgb = if paused { AMBER } else { RED };
    for y in 0..icon.height {
        for x in 0..icon.width {
            let d = ((x as f32 + 0.5 - cx).powi(2) + (y as f32 + 0.5 - cy).powi(2)).sqrt();
            let cover = (radius + 0.5 - d).clamp(0.0, 1.0);
            if cover == 0.0 {
                continue;
            }
            let at = (y * icon.width + x) as usize * 4;
            let Some(px) = out.argb.get_mut(at..at + 4) else {
                continue;
            };
            let below = f32::from(px[0]) / 255.0;
            let alpha = cover + below * (1.0 - cover);
            px[0] = (alpha * 255.0).round() as u8;
            for (i, channel) in rgb.iter().enumerate() {
                let mixed = (f32::from(*channel) * cover
                    + f32::from(px[i + 1]) * below * (1.0 - cover))
                    / alpha;
                px[i + 1] = mixed.round() as u8;
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_icon_renders_at_every_size_and_is_not_blank() {
        let icons = tray_icons();
        assert_eq!(
            icons.iter().map(|i| i.width).collect::<Vec<_>>(),
            [16, 22, 32, 44]
        );
        for icon in &icons {
            assert_eq!(icon.argb.len(), (icon.width * icon.height * 4) as usize);
            assert!(icon.argb.chunks(4).any(|p| p[0] > 200));
        }
    }

    #[test]
    fn the_dot_is_red_in_the_corner_and_amber_when_paused() {
        let idle = &tray_icons()[1];
        let red = with_dot(idle, false);
        let at = (18 * red.width + 18) as usize * 4;
        assert_eq!(red.argb[at..at + 4], [255, RED[0], RED[1], RED[2]]);
        let amber = with_dot(idle, true);
        assert_eq!(amber.argb[at + 1..at + 4], AMBER);
        assert_eq!(
            red.argb[..4],
            idle.argb[..4],
            "the rest of the icon is untouched"
        );
    }
}
