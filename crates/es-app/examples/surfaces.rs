#[cfg(not(feature = "test-hooks"))]
fn main() {
    eprintln!("surfaces: build with --features test-hooks and run under tests/harness/sway.sh");
    std::process::exit(2);
}

#[cfg(feature = "test-hooks")]
fn main() {
    harness::main();
}

#[cfg(feature = "test-hooks")]
mod harness {
    use es_app::platform_ui::{LayerShellSurfaces, SurfaceHandle, SurfaceRole, ToastSurface};
    use es_app::toast::{IconName, TOAST_DURATION, ToastRequest, ToastSink, ToastStyle};
    use es_backend::{Backend, Probe};
    use es_geom::{Point, Size};
    use gpui::{
        App, AppContext as _, Context, IntoElement, ParentElement as _, Render, Styled as _,
        Window, div, px, rgb,
    };
    use std::rc::Rc;
    use std::sync::Arc;
    use std::time::Duration;

    struct Overlay;

    impl Render for Overlay {
        fn render(&mut self, window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
            eprintln!("OVERLAY_FRAME sf={}", window.scale_factor());
            div().size_full().child(
                div()
                    .absolute()
                    .left(px(100.))
                    .top(px(100.))
                    .w(px(200.))
                    .h(px(100.))
                    .bg(rgb(0x0000ff)),
            )
        }
    }

    struct Pin;

    impl Render for Pin {
        fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
            div().size_full().bg(rgb(0x00ff00))
        }
    }

    fn step(label: &str) {
        eprintln!("{label}");
    }

    pub fn main() {
        let probe = match Probe::gather() {
            Ok(p) => p,
            Err(e) => {
                eprintln!("surfaces: {e}");
                std::process::exit(2);
            }
        };
        let Some(test_output) = probe.test.as_ref().map(|t| t.output.clone()) else {
            eprintln!("surfaces: refusing to run outside test mode");
            std::process::exit(2);
        };
        let backend = Arc::new(Backend::detect(&probe));
        gpui_platform::application()
            .with_assets(es_ui::assets::Assets)
            .run(move |cx: &mut App| {
                es_ui::init(cx);
                let surfaces = Rc::new(LayerShellSurfaces::new(backend, Some(test_output.clone())));
                if let Err(e) = surfaces.warm_up(cx) {
                    eprintln!("warm-up: {e}");
                }
                let info = match surfaces.output_info(&test_output) {
                    Ok(info) => info,
                    Err(e) => {
                        eprintln!("surfaces: {e}");
                        std::process::exit(1);
                    }
                };
                eprintln!(
                    "OUTPUT {} logical={:?} scale={}",
                    info.id.0, info.logical, info.scale
                );
                let origin = Point::new(info.logical.x0, info.logical.y0);
                let overlay = surfaces
                    .open_surface(SurfaceRole::Overlay, &test_output, info.logical.size(), cx)
                    .inspect(|h| h.set_content(cx.new(|_| Overlay).into(), cx));
                let pin = surfaces
                    .open_surface(SurfaceRole::Pin, &test_output, Size::new(160., 90.), cx)
                    .inspect(|h| {
                        h.set_content(cx.new(|_| Pin).into(), cx);
                        h.set_position(Point::new(origin.x + 400., origin.y + 300.), cx);
                    });
                let toasts = ToastSurface::new(surfaces.clone());
                let shown = toasts.show(
                    &ToastRequest {
                        title: "Copied to clipboard".into(),
                        message: Some("capture.png".into()),
                        icon: IconName::Copy,
                        style: ToastStyle::Normal,
                        duration: TOAST_DURATION * 2,
                        output: None,
                        undo: None,
                    },
                    cx,
                );
                let (overlay, pin) = match (overlay, pin) {
                    (Ok(o), Ok(p)) if shown => (o, p),
                    (o, p) => {
                        eprintln!(
                            "open failed: overlay {:?} pin {:?} toast {shown}",
                            o.err(),
                            p.err()
                        );
                        std::process::exit(1);
                    }
                };
                cx.spawn(async move |cx| {
                    let wait = |ms| cx.background_executor().timer(Duration::from_millis(ms));
                    wait(1500).await;
                    step("READY");
                    wait(1500).await;
                    cx.update(|cx| {
                        pin.set_position(Point::new(origin.x + 800., origin.y + 500.), cx)
                    });
                    wait(1000).await;
                    step("MOVED");
                    wait(1500).await;
                    cx.update(|cx| {
                        drop(toasts);
                        overlay.close(cx);
                        pin.close(cx);
                        cx.quit();
                    });
                })
                .detach();
            });
    }
}
