use es_model::{
    Alignment, AspectRatio, BackgroundColor, BackgroundGradient, BackgroundSettings,
    BackgroundStyle, CaptureVariant, Config, PresetLibrary, Step, UnitPoint, after_capture_plan,
    resolve_new_capture_look,
};

fn plan(variant: CaptureVariant, edit: impl FnOnce(&mut Config)) -> Vec<Step> {
    let mut cfg = Config::default();
    edit(&mut cfg);
    after_capture_plan(variant, &cfg)
}

#[test]
fn fresh_install_saves_copies_and_shows_the_card() {
    assert_eq!(
        plan(CaptureVariant::Normal, |_| {}),
        [Step::Save, Step::Copy, Step::ShowPreview]
    );
}

#[test]
fn an_upgrade_that_never_enabled_saving_does_not_save() {
    let p = plan(CaptureVariant::Normal, |c| {
        c.after_capture.screenshot.save = false
    });
    assert!(!p.contains(&Step::Save));
    assert!(p.contains(&Step::ShowPreview));
}

#[test]
fn region_copy_always_copies_and_never_auto_saves() {
    let p = plan(CaptureVariant::RegionCopy, |c| {
        c.general.copy_after_save = false;
        c.after_capture.screenshot.save = true;
    });
    assert_eq!(p, [Step::Copy, Step::ShowPreview]);
}

#[test]
fn region_save_always_saves_and_never_copies() {
    let p = plan(CaptureVariant::RegionSave, |c| {
        c.after_capture.screenshot.save = false;
        c.general.copy_after_save = true;
        c.after_capture.screenshot.copy = true;
    });
    assert_eq!(p, [Step::Save, Step::ShowPreview]);
}

#[test]
fn region_annotate_opens_the_editor_without_saving() {
    let p = plan(CaptureVariant::RegionAnnotate, |c| {
        c.after_capture.screenshot.save = true;
    });
    assert_eq!(p, [Step::Copy, Step::ShowPreview, Step::OpenEditor]);
}

#[test]
fn region_pin_pins_and_leaves_no_card() {
    let p = plan(CaptureVariant::RegionPin, |_| {});
    assert_eq!(p, [Step::Copy, Step::Pin]);
    assert!(!p.contains(&Step::ShowPreview));
}

#[test]
fn copy_is_off_only_when_both_copy_switches_are_off() {
    let off = |c: &mut Config| c.general.copy_after_save = false;
    assert!(!plan(CaptureVariant::Normal, off).contains(&Step::Copy));
    let matrix_on = |c: &mut Config| {
        c.general.copy_after_save = false;
        c.after_capture.screenshot.copy = true;
    };
    assert!(plan(CaptureVariant::Normal, matrix_on).contains(&Step::Copy));
}

#[test]
fn the_editor_opens_for_every_variant_but_region_copy_when_asked() {
    let ask = |c: &mut Config| c.capture.open_editor_after_capture = true;
    assert!(plan(CaptureVariant::Normal, ask).contains(&Step::OpenEditor));
    assert!(plan(CaptureVariant::RegionSave, ask).contains(&Step::OpenEditor));
    assert!(!plan(CaptureVariant::RegionCopy, ask).contains(&Step::OpenEditor));
    let matrix = |c: &mut Config| c.after_capture.screenshot.open_editor = true;
    assert!(plan(CaptureVariant::Normal, matrix).contains(&Step::OpenEditor));
}

#[test]
fn the_pin_switch_pins_plain_captures_only() {
    let pin = |c: &mut Config| c.after_capture.screenshot.pin = true;
    assert!(plan(CaptureVariant::Normal, pin).contains(&Step::Pin));
    assert!(!plan(CaptureVariant::RegionCopy, pin).contains(&Step::Pin));
    assert!(!plan(CaptureVariant::RegionSave, pin).contains(&Step::Pin));
}

#[test]
fn the_card_can_be_switched_off() {
    let p = plan(CaptureVariant::Normal, |c| {
        c.after_capture.screenshot.show_preview = false
    });
    assert_eq!(p, [Step::Save, Step::Copy]);
}

#[test]
fn steps_run_in_save_copy_card_pin_editor_order() {
    let p = plan(CaptureVariant::Normal, |c| {
        c.after_capture.screenshot.pin = true;
        c.after_capture.screenshot.open_editor = true;
    });
    assert_eq!(
        p,
        [
            Step::Save,
            Step::Copy,
            Step::ShowPreview,
            Step::Pin,
            Step::OpenEditor
        ]
    );
}

fn cobalt() -> BackgroundColor {
    BackgroundColor::new("cobalt", "Cobalt", 0.16, 0.50, 0.88)
}

#[test]
fn the_default_config_gives_no_background_and_the_documented_numbers() {
    let look = resolve_new_capture_look(&Config::default(), &PresetLibrary::default());
    assert_eq!(look, BackgroundSettings::default());
    assert_eq!(look.style, BackgroundStyle::None);
}

#[test]
fn padding_corner_radius_and_shadow_carry_over_from_the_config() {
    let mut cfg = Config::default();
    cfg.default_look.padding = 0.18;
    cfg.default_look.corner_radius = 0.12;
    cfg.default_look.shadow = 0.5;
    let look = resolve_new_capture_look(&cfg, &PresetLibrary::default());
    assert_eq!(
        (look.padding, look.corner_radius, look.shadow),
        (0.18, 0.12, 0.5)
    );
    assert_eq!(
        (look.alignment, look.aspect_ratio),
        (Alignment::Center, AspectRatio::Auto)
    );
}

#[test]
fn a_named_preset_carries_style_alignment_and_aspect_ratio() {
    let mut lib = PresetLibrary::default();
    let bg = BackgroundSettings {
        style: BackgroundStyle::Solid(cobalt()),
        padding: 0.18,
        corner_radius: 0.18,
        shadow: 0.5,
        alignment: Alignment::BottomLeading,
        aspect_ratio: AspectRatio::SixteenNine,
        ..BackgroundSettings::default()
    };
    lib.add("Cobalt", bg.clone());
    lib.active_preset_id = None;
    let mut cfg = Config::default();
    cfg.default_look.style = "cobalt".into();
    let look = resolve_new_capture_look(&cfg, &lib);
    assert_eq!(look, bg);
    match look.style {
        BackgroundStyle::Solid(c) => assert!((c.red - 0.16).abs() < 1e-4 && c.id == "cobalt"),
        other => panic!("expected a solid style, got {other:?}"),
    }
}

#[test]
fn a_custom_colour_survives_the_look_round_trip() {
    let custom = BackgroundColor::custom(0.2, 0.4, 0.6);
    let mut lib = PresetLibrary::default();
    lib.add(
        "Custom",
        BackgroundSettings {
            style: BackgroundStyle::Solid(custom.clone()),
            ..BackgroundSettings::default()
        },
    );
    let stored = serde_json::to_string(&lib).unwrap();
    let lib: PresetLibrary = serde_json::from_str(&stored).unwrap();
    match resolve_new_capture_look(&Config::default(), &lib).style {
        BackgroundStyle::Solid(c) => {
            assert_eq!(c, custom);
            assert_eq!(c.id, "custom");
            assert!((c.green - 0.4).abs() < 1e-4 && c.alpha == 1.0);
        }
        other => panic!("expected a solid style, got {other:?}"),
    }
}

#[test]
fn every_gradient_stop_and_both_end_points_carry_over() {
    let gradient = BackgroundGradient {
        id: "dusk".into(),
        title: "Dusk".into(),
        colors: BackgroundColor::plain_presets()
            .into_iter()
            .take(4)
            .collect(),
        start: UnitPoint { x: 0.0, y: 0.25 },
        end: UnitPoint { x: 1.0, y: 0.75 },
    };
    let mut lib = PresetLibrary::default();
    lib.add(
        "Dusk",
        BackgroundSettings {
            style: BackgroundStyle::Gradient(gradient.clone()),
            ..BackgroundSettings::default()
        },
    );
    match resolve_new_capture_look(&Config::default(), &lib).style {
        BackgroundStyle::Gradient(g) => {
            assert_eq!(g.id, "dusk");
            assert_eq!(g.colors.len(), 4);
            assert_eq!((g.start, g.end), (gradient.start, gradient.end));
        }
        other => panic!("expected a gradient, got {other:?}"),
    }
}

#[test]
fn a_wallpaper_preset_keeps_its_path() {
    let mut lib = PresetLibrary::default();
    lib.add(
        "Wall",
        BackgroundSettings {
            style: BackgroundStyle::Wallpaper {
                path: "/tmp/wall.jpg".into(),
            },
            ..BackgroundSettings::default()
        },
    );
    match resolve_new_capture_look(&Config::default(), &lib).style {
        BackgroundStyle::Wallpaper { path } => {
            assert_eq!(path, std::path::PathBuf::from("/tmp/wall.jpg"))
        }
        other => panic!("expected a wallpaper, got {other:?}"),
    }
}

#[test]
fn a_style_naming_no_preset_falls_back_to_no_background_with_the_numbers() {
    let mut cfg = Config::default();
    cfg.default_look.style = "deleted preset".into();
    cfg.default_look.padding = 0.2;
    let look = resolve_new_capture_look(&cfg, &PresetLibrary::default());
    assert_eq!(look.style, BackgroundStyle::None);
    assert_eq!(look.padding, 0.2);
}

#[test]
fn the_active_preset_overrides_the_config_look() {
    let mut lib = PresetLibrary::default();
    let bg = BackgroundSettings {
        style: BackgroundStyle::Solid(cobalt()),
        padding: 0.3,
        ..BackgroundSettings::default()
    };
    lib.add("Active", bg.clone());
    let mut cfg = Config::default();
    cfg.default_look.padding = 0.01;
    assert_eq!(resolve_new_capture_look(&cfg, &lib), bg);
}
