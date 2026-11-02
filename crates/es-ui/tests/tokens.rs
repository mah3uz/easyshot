use es_geom::Rgba;
use es_ui::tokens::{Appearance, DARK, LIGHT, METRICS, TYPE, ThemeMode, Tokens};
use gpui::{FontWeight, px};

fn rgb(c: Rgba) -> u32 {
    let byte = |v: f32| (v * 255.).round() as u32;
    byte(c.r) << 16 | byte(c.g) << 8 | byte(c.b)
}

fn solid(c: Rgba) -> u32 {
    assert_eq!(c.a, 1., "{c:?} should be opaque");
    rgb(c)
}

fn alpha_of(c: Rgba, base: u32) -> f32 {
    assert_eq!(rgb(c), base, "{c:?} is the wrong base colour");
    c.a
}

#[test]
fn dark_chrome_matches_the_measured_values() {
    let d = DARK;
    assert_eq!(solid(d.window), 0x1B1B1B);
    assert_eq!(solid(d.workspace), 0x1B1B1B);
    assert_eq!(solid(d.inspector), 0x2B2B2B);
    assert_eq!(solid(d.track), 0x363636);
    assert_eq!(solid(d.value_box), 0x363636);
    assert_eq!(solid(d.segment_track), 0x363636);
    assert_eq!(solid(d.fill), 0x494949);
    assert_eq!(solid(d.selected_segment), 0x494949);
    assert_eq!(solid(d.dropdown), 0x444444);
    assert_eq!(solid(d.separator), 0x464646);
    assert_eq!(alpha_of(d.border, 0xFFFFFF), 0.10);
    assert_eq!(solid(d.accent), 0x008EFB);
    assert_eq!(solid(d.selection), 0x3182ED);
    assert_eq!(solid(d.accent_tint), 0x456383);
    assert_eq!(alpha_of(d.text, 0xFFFFFF), 0.92);
    assert_eq!(alpha_of(d.text_secondary, 0xFFFFFF), 0.55);
    assert_eq!(alpha_of(d.text_tertiary, 0xFFFFFF), 0.26);
    assert_eq!(solid(d.destructive), 0xFF453A);
    assert_eq!(solid(d.success), 0x32D74B);
}

#[test]
fn light_chrome_is_white_and_the_workspace_grey() {
    let l = LIGHT;
    assert_eq!(
        solid(l.window),
        0xFFFFFF,
        "chrome follows the measured white"
    );
    assert_eq!(
        solid(l.workspace),
        0xECECEC,
        "the workspace keeps the system grey"
    );
    assert_eq!(solid(l.inspector), 0xEAEAEA);
    assert_eq!(solid(l.track), 0xE0E0E0);
    assert_eq!(solid(l.segment_track), 0xE0E0E0);
    assert_eq!(solid(l.value_box), 0xD3D3D3);
    assert_eq!(solid(l.fill), 0x9F9F9F);
    assert_eq!(solid(l.selected_segment), 0xFFFFFF);
    assert_eq!(solid(l.dropdown), 0xFFFFFF);
    assert_eq!(alpha_of(l.separator, 0x000000), 0.10);
    assert_eq!(alpha_of(l.border, 0x000000), 0.10);
    assert_eq!(solid(l.accent), 0x1184F6);
    assert_eq!(solid(l.selection), 0x3182ED);
    assert_eq!(alpha_of(l.accent_tint, 0x1184F6), 0.12);
    assert_eq!(alpha_of(l.text, 0x000000), 0.88);
    assert_eq!(alpha_of(l.text_secondary, 0x000000), 0.50);
    assert_eq!(alpha_of(l.text_tertiary, 0x000000), 0.26);
    assert_eq!(solid(l.destructive), 0xD70015);
    assert_eq!(solid(l.success), 0x248A3D);
}

#[test]
fn button_states_are_the_primary_colour_at_fixed_strengths() {
    for (palette, base) in [(DARK, 0xFFFFFF), (LIGHT, 0x000000)] {
        assert_eq!(alpha_of(palette.hover_fill, base), 0.10);
        assert_eq!(alpha_of(palette.hover_border, base), 0.25);
        assert_eq!(alpha_of(palette.press_fill, base), 0.16);
        assert_eq!(alpha_of(palette.bordered_fill, base), 0.05);
        assert_eq!(alpha_of(palette.bordered_border, base), 0.15);
        assert_eq!(solid(palette.on_accent), 0xFFFFFF);
    }
}

// A colour left out of the light palette would silently keep its dark value.
#[test]
fn only_the_shared_colours_are_the_same_in_both_palettes() {
    let shared = ["selection", "on_accent"];
    for ((name, dark), (_, light)) in DARK.named().into_iter().zip(LIGHT.named()) {
        if shared.contains(&name) {
            assert_eq!(dark, light, "{name}");
        } else {
            assert_ne!(dark, light, "{name} is the same in both palettes");
        }
    }
}

#[test]
fn metrics_match_the_inspector_layout() {
    let m = METRICS;
    assert_eq!(m.inspector_padding, px(12.));
    assert_eq!(m.section_padding, px(12.));
    assert_eq!(m.row_spacing, px(8.));
    assert_eq!(m.control_height, px(24.));
    assert_eq!(
        m.slider_height,
        px(32.),
        "the source's 32, not the scaled video's 27"
    );
    assert_eq!(m.field_radius, px(5.));
    assert_eq!(m.slider_radius, px(8.));
    assert_eq!(m.tile_radius, px(6.));
    assert_eq!(m.header_height, px(38.));
    assert_eq!(m.button_height, px(32.));
    assert_eq!(m.button_radius, px(6.));
    assert_eq!(m.slider_value_width, px(60.));
    assert_eq!(m.popover_radius, px(12.));
}

#[test]
fn the_type_scale_is_eleven_to_thirteen_pixels() {
    let s = TYPE;
    let spec = |t: es_ui::tokens::TextStyle| (f32::from(t.size), t.weight, t.tabular);
    assert_eq!(spec(s.section_header), (11., FontWeight::SEMIBOLD, false));
    assert_eq!(spec(s.label), (11., FontWeight::NORMAL, false));
    assert_eq!(
        spec(s.value),
        (11., FontWeight::MEDIUM, true),
        "values use tabular digits so they do not jitter while scrubbing"
    );
    assert_eq!(spec(s.tool_button), (12., FontWeight::MEDIUM, false));
    assert_eq!(spec(s.title), (13., FontWeight::SEMIBOLD, false));
}

#[test]
fn system_follows_the_desktop_and_is_dark_without_a_preference() {
    use Appearance::{Dark, Light};
    assert_eq!(ThemeMode::System(Some(Light)).appearance(), Light);
    assert_eq!(ThemeMode::System(Some(Dark)).appearance(), Dark);
    assert_eq!(ThemeMode::System(None).appearance(), Dark);
    assert_eq!(ThemeMode::Light.appearance(), Light);
    assert_eq!(ThemeMode::Dark.appearance(), Dark);
}

#[test]
fn tokens_are_the_palette_of_their_mode() {
    let light = Tokens::for_mode(ThemeMode::System(Some(Appearance::Light)));
    assert_eq!(light.appearance, Appearance::Light);
    let window = light.window.to_rgb();
    assert!(window.r.min(window.g).min(window.b) > 0.999);
    let dark = Tokens::new(Appearance::Dark);
    assert!(dark.text.l > 0.9 && light.text.l < 0.1);
}
