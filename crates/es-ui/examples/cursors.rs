use std::{path::PathBuf, sync::Arc};

use es_ui::cursors::{self, Cursors};
use gpui::{
    App, AppContext, Bounds, Context, CursorStyle, Edges, ExternalDragPayload, FileDragPaths,
    Image, ImageFormat, InteractiveElement, IntoElement, MouseButton, ParentElement, Render,
    RenderImage, SharedString, StatefulInteractiveElement, Styled, Window, WindowBounds,
    WindowKind, WindowOptions, div,
    layer_shell::{Anchor, KeyboardInteractivity, Layer, LayerShellOptions},
    point, px, rgb, size,
};

const TILE: f32 = 100.0;
const GAP: f32 = 10.0;
const ICON: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" width="240" height="150"><rect width="240" height="150" rx="16" fill="#e8590c"/><circle cx="120" cy="75" r="40" fill="#ffffff"/><rect x="20" y="20" width="60" height="20" rx="4" fill="#1c7ed6"/></svg>"##;

fn main() {
    gpui_platform::application()
        .with_assets(es_ui::assets::Assets)
        .run(|cx: &mut App| {
            es_ui::fonts::register(cx);
            let cursors = cursors::register(cx);
            let icon = Image::from_bytes(ImageFormat::Svg, ICON.as_bytes().to_vec())
                .to_image_data(cx.svg_renderer());
            let icon = match icon {
                Ok(icon) => icon,
                Err(error) => {
                    eprintln!("cannot render the drag icon: {error}");
                    cx.quit();
                    return;
                }
            };
            let main = WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds::new(
                    point(px(0.), px(0.)),
                    size(px(560.), px(360.)),
                ))),
                app_id: Some("es-cursors".into()),
                ..WindowOptions::default()
            };
            let layer = WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds::new(
                    point(px(0.), px(0.)),
                    size(px(240.), px(80.)),
                ))),
                kind: WindowKind::LayerShell(LayerShellOptions {
                    namespace: "es-cursors-layer".into(),
                    layer: Layer::Top,
                    anchor: Anchor::TOP | Anchor::LEFT,
                    exclusive_zone: None,
                    exclusive_edge: None,
                    margin: Some((px(0.), px(0.), px(0.), px(0.))),
                    keyboard_interactivity: KeyboardInteractivity::None,
                }),
                app_id: Some("es-cursors-layer".into()),
                ..WindowOptions::default()
            };
            let opened = cx
                .open_window(main, |_, cx| cx.new(|_| Tiles { cursors, icon }))
                .and_then(|_| cx.open_window(layer, |_, cx| cx.new(|_| LayerPanel { left: 0.0 })));
            if let Err(error) = opened {
                eprintln!("cannot open the test windows: {error}");
                cx.quit();
            }
        });
}

struct Tiles {
    cursors: Cursors,
    icon: Arc<RenderImage>,
}

#[derive(Clone)]
struct Dragged;

struct DragGhost;

impl Render for DragGhost {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().size(px(40.)).bg(rgb(0x1c7ed6))
    }
}

fn tile(index: usize, label: SharedString, style: CursorStyle) -> impl IntoElement {
    let (column, row) = ((index % 5) as f32, (index / 5) as f32);
    div()
        .absolute()
        .left(px(GAP + column * (TILE + GAP)))
        .top(px(GAP + row * (TILE + GAP)))
        .size(px(TILE))
        .bg(rgb(0xdee2e6))
        .text_color(rgb(0x212529))
        .text_sm()
        .p_1()
        .cursor(style)
        .child(label)
}

impl Render for Tiles {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let c = self.cursors;
        let mut tiles = vec![
            tile(
                0,
                "annotationPlus".into(),
                CursorStyle::Custom(c.annotation_plus),
            )
            .into_any_element(),
            tile(
                1,
                "windowCamera".into(),
                CursorStyle::Custom(c.window_camera),
            )
            .into_any_element(),
        ];
        tiles.extend(c.rotate.iter().enumerate().map(|(k, id)| {
            tile(
                k + 2,
                format!("rotate {}", k * 45).into(),
                CursorStyle::Custom(*id),
            )
            .into_any_element()
        }));
        let icon = self.icon.clone();
        let path = std::env::current_exe().unwrap_or_else(|_| PathBuf::from("/"));
        div()
            .size_full()
            .relative()
            .bg(rgb(0xf8f9fa))
            .font_family("Inter")
            .children(tiles)
            .child(
                div()
                    .id("drag")
                    .absolute()
                    .left(px(GAP))
                    .top(px(GAP + 2.0 * (TILE + GAP)))
                    .w(px(2.0 * TILE + GAP))
                    .h(px(TILE))
                    .bg(rgb(0xffd8a8))
                    .text_color(rgb(0x212529))
                    .p_1()
                    .child("drag me out")
                    .on_drag(Dragged, |_, _, _, cx| cx.new(|_| DragGhost))
                    .external_drag_payload(move |_: &Dragged, _, _| {
                        Some(ExternalDragPayload::Files(
                            FileDragPaths::new([(path.clone(), false)]).with_icon(icon.clone()),
                        ))
                    }),
            )
    }
}

struct LayerPanel {
    left: f32,
}

impl Render for LayerPanel {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .bg(rgb(0x2f9e44))
            .text_color(rgb(0xffffff))
            .p_2()
            .child(format!("layer, left margin {}", self.left))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, window, cx| {
                    this.left = if this.left >= 600.0 {
                        0.0
                    } else {
                        this.left + 300.0
                    };
                    window.set_layer_margin(Edges {
                        left: px(this.left),
                        ..Edges::default()
                    });
                    eprintln!("layer margin left {}", this.left);
                    cx.notify();
                }),
            )
    }
}
