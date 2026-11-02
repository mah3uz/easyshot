use es_app::platform_ui::layer::{
    check_test_output, find_display, layer_options, margins, namespace, output_uuid,
    passes_input_through, scale_ready, wants_exclusive,
};
use es_app::platform_ui::{Gate, SurfaceRole, card_icon, with_file_items};
use es_geom::{Point, Rect, Size};
use es_model::platform::{ClipItem, OutputId};
use gpui::layer_shell::{Anchor, KeyboardInteractivity as K, Layer};
use gpui::px;
use std::cell::Cell;
use std::path::PathBuf;
use std::sync::Arc;
use uuid::Uuid;

const ROLES: [SurfaceRole; 8] = [
    SurfaceRole::Overlay,
    SurfaceRole::Countdown,
    SurfaceRole::Toast,
    SurfaceRole::Deck,
    SurfaceRole::Pin,
    SurfaceRole::Bar,
    SurfaceRole::Highlight,
    SurfaceRole::Scroll,
];

fn out(name: &str) -> OutputId {
    OutputId(name.into())
}

#[test]
fn every_surface_carries_an_easyshot_namespace_for_compositor_rules() {
    for role in ROLES {
        let o = layer_options(role, false);
        assert!(o.namespace.starts_with("easyshot-"), "{role:?}");
        assert_eq!(o.namespace, namespace(role));
    }
}

#[test]
fn the_overlay_covers_its_whole_output_above_everything() {
    let o = layer_options(SurfaceRole::Overlay, false);
    assert_eq!(o.layer, Layer::Overlay);
    assert_eq!(o.anchor, Anchor::all());
    assert_eq!(o.exclusive_zone, Some(px(-1.)));
    assert_eq!(o.keyboard_interactivity, K::OnDemand);
    let grabbing = layer_options(SurfaceRole::Overlay, true);
    assert_eq!(grabbing.keyboard_interactivity, K::Exclusive);
}

#[test]
fn only_an_overlay_under_the_pointer_outside_test_mode_grabs_the_keyboard() {
    let (a, b) = (out("DP-1"), out("DP-2"));
    assert!(wants_exclusive(SurfaceRole::Overlay, None, Some(&a), &a));
    assert!(!wants_exclusive(SurfaceRole::Overlay, None, Some(&b), &a));
    assert!(!wants_exclusive(SurfaceRole::Overlay, None, None, &a));
    assert!(!wants_exclusive(SurfaceRole::Pin, None, Some(&a), &a));
    assert!(!wants_exclusive(
        SurfaceRole::Overlay,
        Some(&a),
        Some(&a),
        &a
    ));
    for role in ROLES {
        let exclusive = wants_exclusive(role, Some(&a), Some(&a), &a);
        assert_ne!(
            layer_options(role, exclusive).keyboard_interactivity,
            K::Exclusive
        );
    }
}

#[test]
fn passive_surfaces_never_take_keyboard_or_pointer() {
    for role in [
        SurfaceRole::Countdown,
        SurfaceRole::Highlight,
        SurfaceRole::Scroll,
    ] {
        assert_eq!(layer_options(role, true).keyboard_interactivity, K::None);
        assert!(passes_input_through(role), "{role:?}");
    }
    assert_eq!(
        layer_options(SurfaceRole::Toast, false).keyboard_interactivity,
        K::None
    );
    for role in [SurfaceRole::Toast, SurfaceRole::Deck, SurfaceRole::Pin] {
        assert!(!passes_input_through(role), "{role:?}");
    }
}

#[test]
fn each_role_sits_on_its_layer_and_edges() {
    let cases = [
        (SurfaceRole::Countdown, Layer::Overlay, Anchor::empty()),
        (SurfaceRole::Toast, Layer::Top, Anchor::TOP),
        (
            SurfaceRole::Deck,
            Layer::Top,
            Anchor::BOTTOM | Anchor::RIGHT,
        ),
        (SurfaceRole::Pin, Layer::Top, Anchor::TOP | Anchor::LEFT),
        (SurfaceRole::Bar, Layer::Top, Anchor::BOTTOM),
        (
            SurfaceRole::Highlight,
            Layer::Overlay,
            Anchor::TOP | Anchor::LEFT,
        ),
        (
            SurfaceRole::Scroll,
            Layer::Overlay,
            Anchor::TOP | Anchor::LEFT,
        ),
    ];
    for (role, layer, anchor) in cases {
        let o = layer_options(role, false);
        assert_eq!((o.layer, o.anchor), (layer, anchor), "{role:?}");
    }
}

#[test]
fn surfaces_placed_by_margins_ignore_other_surfaces_reserved_space() {
    for role in [
        SurfaceRole::Pin,
        SurfaceRole::Deck,
        SurfaceRole::Highlight,
        SurfaceRole::Scroll,
    ] {
        assert_eq!(layer_options(role, false).exclusive_zone, Some(px(-1.)));
    }
}

#[test]
fn an_output_is_matched_by_the_uuid_gpui_derives_from_its_name() {
    let want = Uuid::new_v5(&Uuid::NAMESPACE_DNS, b"HEADLESS-1");
    assert_eq!(output_uuid(&out("HEADLESS-1")), want);
    let displays = [
        (1, Some(output_uuid(&out("DP-2")))),
        (2, None),
        (3, Some(want)),
    ];
    assert_eq!(find_display(displays, &out("HEADLESS-1")).ok(), Some(3));
}

#[test]
fn no_matching_display_is_an_error_never_another_screen() {
    let displays = [(1, Some(output_uuid(&out("DP-2")))), (2, None)];
    assert!(find_display(displays, &out("HEADLESS-1")).is_err());
    assert!(find_display(Vec::<(u8, Option<Uuid>)>::new(), &out("DP-2")).is_err());
}

#[test]
fn test_mode_refuses_every_output_but_the_test_one() {
    let test = out("HEADLESS-1");
    assert!(check_test_output(Some(&test), &out("DP-2")).is_err());
    assert!(check_test_output(Some(&test), &test).is_ok());
    assert!(check_test_output(None, &out("DP-2")).is_ok());
}

#[test]
fn a_pin_at_scale_1_gets_its_position_as_top_left_margins() {
    let output = Rect::new(0., 0., 1920., 1080.);
    let size = Size::new(200., 100.);
    let m = margins(
        Anchor::TOP | Anchor::LEFT,
        output,
        size,
        Point::new(300., 400.),
    );
    assert_eq!(
        (m.top, m.left, m.right, m.bottom),
        (px(400.), px(300.), px(0.), px(0.))
    );
}

#[test]
fn margins_are_logical_and_relative_to_the_outputs_origin_at_scale_1_5() {
    // 1920x1080 at scale 1.5, right of a 1920-wide output.
    let output = Rect::new(1920., 0., 3200., 720.);
    let size = Size::new(200., 100.);
    let pin = margins(
        Anchor::TOP | Anchor::LEFT,
        output,
        size,
        Point::new(2000.4, 99.6),
    );
    assert_eq!((pin.top, pin.left), (px(100.), px(80.)));
    let deck = margins(
        Anchor::BOTTOM | Anchor::RIGHT,
        output,
        size,
        Point::new(3000., 600.),
    );
    assert_eq!((deck.right, deck.bottom), (px(0.), px(20.)));
    assert_eq!((deck.top, deck.left), (px(0.), px(0.)));
}

#[test]
fn an_output_left_of_the_origin_still_gives_positive_margins() {
    let output = Rect::new(-1280., -200., 0., 520.);
    let m = margins(
        Anchor::TOP | Anchor::LEFT,
        output,
        Size::new(10., 10.),
        Point::new(-1180., -150.),
    );
    assert_eq!((m.top, m.left), (px(50.), px(100.)));
}

#[test]
fn the_gate_holds_until_the_window_scale_matches_the_output() {
    assert!(!scale_ready(1.0, 1.5));
    assert!(scale_ready(1.5, 1.5));
    assert!(scale_ready(1.0, 1.0));
    assert!(!scale_ready(2.0, 1.5));
    let gate = Gate::new(1.5);
    assert!(!gate.ready(1.0));
    assert!(gate.ready(1.5));
}

#[test]
fn a_stale_scale_never_keeps_a_surface_blank() {
    let gate = Gate::new(1.5);
    std::thread::sleep(std::time::Duration::from_millis(300));
    assert!(gate.ready(1.0));
}

#[test]
fn an_image_on_the_clipboard_also_offers_its_file() {
    let png: Arc<[u8]> = Arc::from(&b"png"[..]);
    let file = PathBuf::from("/tmp/x/capture.png");
    let items = with_file_items(vec![ClipItem::Png(png.clone())], |bytes| {
        assert_eq!(bytes, b"png");
        Ok(file.clone())
    });
    assert_eq!(
        items.ok(),
        Some(vec![
            ClipItem::Png(png),
            ClipItem::UriList(vec![file.clone()]),
            ClipItem::GnomeCopiedFiles(vec![file]),
        ])
    );
}

#[test]
fn items_that_already_name_files_or_carry_no_image_are_left_alone() {
    let written = Cell::new(false);
    let given = vec![
        ClipItem::Png(Arc::from(&b"png"[..])),
        ClipItem::UriList(vec![PathBuf::from("/a.png")]),
    ];
    let items = with_file_items(given.clone(), |_| {
        written.set(true);
        Ok(PathBuf::new())
    });
    assert_eq!(items.ok(), Some(given));
    let text = vec![ClipItem::Text("hi".into())];
    assert_eq!(
        with_file_items(text.clone(), |_| {
            written.set(true);
            Ok(PathBuf::new())
        })
        .ok(),
        Some(text)
    );
    assert!(!written.get());
}

#[test]
fn toast_icons_reach_the_card() {
    use gpui_component::IconName as Api;
    assert_eq!(
        card_icon(Api::TriangleAlert),
        Some(es_ui::icons::IconName::TriangleAlert)
    );
}

struct TwoOutputs;

impl es_app::platform_ui::OutputSource for TwoOutputs {
    fn outputs(&self) -> Vec<es_model::platform::OutputInfo> {
        ["DP-1", "HDMI-1"]
            .iter()
            .map(|name| es_model::platform::OutputInfo {
                id: out(name),
                description: String::new(),
                logical: Rect::new(0., 0., 100., 100.),
                physical: (100, 100),
                scale: 1.0,
                transform: es_model::platform::Transform::Normal,
                refresh_mhz: 60_000,
                focused: None,
            })
            .collect()
    }
    fn pointer_output(&self) -> Option<OutputId> {
        Some(out("DP-1"))
    }
    fn focused_output(&self) -> Option<OutputId> {
        Some(out("DP-1"))
    }
    fn pointer_global(&self) -> Option<Point> {
        None
    }
}

#[test]
fn the_selector_opens_on_the_pinned_screen_only_when_it_is_chosen_and_connected() {
    use es_app::platform_ui::{OutputSource, PinnedOutput, PinnedOutputSource};
    let pinned = PinnedOutput::default();
    let source = PinnedOutputSource::new(Arc::new(TwoOutputs), pinned.clone());
    assert_eq!(source.pointer_output(), Some(out("DP-1")));
    pinned.set(false, "HDMI-1");
    assert_eq!(source.pointer_output(), Some(out("HDMI-1")));
    pinned.set(true, "HDMI-1");
    assert_eq!(source.pointer_output(), Some(out("DP-1")));
    pinned.set(false, "unplugged");
    assert_eq!(source.pointer_output(), Some(out("DP-1")));
}
