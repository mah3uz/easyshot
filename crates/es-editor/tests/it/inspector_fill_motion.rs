#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::inspector_background::window;

use es_editor::{
    EditorWindow,
    inspector::{
        Section,
        background::{BackgroundBody, Library, WallpaperTab, wallpaper_cache::Source},
    },
};
use es_testkit::Headless;
use es_ui::{
    motion::{FILL_SWITCH, set_reduce_motion},
    tokens::ThemeMode,
};
use gpui::Entity;
use gpui_component::Root;
use image::RgbaImage;

struct Rig {
    app: Headless<Root>,
    body: Entity<BackgroundBody>,
    _editor: Entity<EditorWindow>,
}

fn rig(reduce: bool) -> Rig {
    let (mut app, editor) = window(ThemeMode::Dark);
    let body = app.cx.update(|cx| {
        set_reduce_motion(reduce, cx);
        let inspector = editor.read(cx).inspector().unwrap();
        let view = inspector
            .read(cx)
            .body(Section::Background)
            .unwrap()
            .clone();
        view.downcast::<BackgroundBody>().unwrap()
    });
    app.settle();
    Rig {
        app,
        body,
        _editor: editor,
    }
}

impl Rig {
    fn change(&mut self, f: impl FnOnce(&mut BackgroundBody, &mut gpui::Context<BackgroundBody>)) {
        let body = self.body.clone();
        self.app.cx.update(|cx| body.update(cx, f));
        self.app.settle();
    }

    fn frame(&mut self, name: &str) -> RgbaImage {
        self.change(|_, cx| cx.notify());
        self.app.screenshot(name)
    }

    /// Frames at the start, halfway, three quarters of the way and at the end of the switch.
    fn frames(&mut self, name: &str) -> [RgbaImage; 4] {
        let quarter = FILL_SWITCH.duration / 4;
        let start = self.frame(&format!("{name}-0-start"));
        self.app.cx.advance_clock(quarter * 2);
        let mid = self.frame(&format!("{name}-1-mid"));
        self.app.cx.advance_clock(quarter);
        let late = self.frame(&format!("{name}-2-late"));
        self.app.cx.advance_clock(quarter);
        let end = self.frame(&format!("{name}-3-end"));
        [start, mid, late, end]
    }

    fn built_in_ready(&mut self) {
        let cache = self.app.cx.update(|cx| self.body.read(cx).cache().clone());
        self.app.until("the built-in thumbnails", |cx| {
            (0..7).all(|i| cache.read(cx).is_ready(&Source::BuiltIn(i)))
        });
    }
}

fn distance(a: &RgbaImage, b: &RgbaImage) -> u64 {
    a.pixels()
        .zip(b.pixels())
        .map(|(p, q)| {
            p.0.iter()
                .zip(q.0)
                .map(|(x, y)| u64::from(x.abs_diff(y)))
                .sum::<u64>()
        })
        .sum()
}

fn assert_fades_in(frames: &[RgbaImage; 4], settled: &RgbaImage, what: &str) {
    let [start, mid, late, end] = frames;
    let left = distance(mid, end) as f32 / distance(start, end) as f32;
    let expected = 1. - FILL_SWITCH.easing.at(0.5);
    assert!(
        (left - expected).abs() < 0.02,
        "{what} has {left} of its fade left halfway, as its token says ({expected})"
    );
    assert!(
        distance(late, end) > 0,
        "{what} is still fading three quarters of the way"
    );
    assert!(end == settled, "{what} arrives when its duration is up");
}

#[test]
fn switching_the_fill_library_fades_the_new_swatches_in() {
    let mut r = rig(false);
    let before = r.frame("fill-switch-before");
    r.change(|b, cx| b.set_library(Library::Color, cx));
    assert_eq!(
        r.app.cx.update(|cx| r.body.read(cx).library()),
        Library::Color,
        "the library changes at once; only its drawing fades"
    );
    let frames = r.frames("fill-switch");
    assert!(frames[0] != before, "the old swatches leave at once");

    let mut still = rig(true);
    still.change(|b, cx| b.set_library(Library::Color, cx));
    let settled = still.frame("fill-switch-reduced");
    assert_fades_in(&frames, &settled, "the colour swatches");
}

#[test]
fn switching_the_wallpaper_source_fades_the_new_grid_in() {
    let open_built_in = |r: &mut Rig| {
        r.change(|b, cx| {
            b.set_library(Library::Wallpaper, cx);
            b.set_wallpaper_tab(WallpaperTab::BuiltIn, cx);
        });
        r.built_in_ready();
        r.change(|b, cx| b.set_wallpaper_tab(WallpaperTab::Recent, cx));
        r.app.cx.advance_clock(FILL_SWITCH.duration * 2);
        r.frame("wallpaper-switch-before");
        r.change(|b, cx| b.set_wallpaper_tab(WallpaperTab::BuiltIn, cx));
    };
    let mut r = rig(false);
    open_built_in(&mut r);
    let frames = r.frames("wallpaper-switch");

    let mut still = rig(true);
    open_built_in(&mut still);
    let settled = still.frame("wallpaper-switch-reduced");
    assert_fades_in(&frames, &settled, "the built-in wallpapers");
}
