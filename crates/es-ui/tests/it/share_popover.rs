use std::{cell::RefCell, rc::Rc};

use crate::support::{self, App, click, key};
use es_testkit::Headless;
use es_ui::assets::Assets;
use es_ui::components::{
    ButtonKind, EditorButton, Popover, SHARE_POPOVER_WIDTH, ShareEvent, SharePopover, ShareView,
};
use es_ui::icons::IconName;
use es_ui::theme::tokens;
use es_ui::tokens::ThemeMode;
use gpui::{
    AppContext, Context, Entity, Focusable, IntoElement, KeyUpEvent, ParentElement, PlatformInput,
    Render, Rgba, Styled, Window, div, px,
};
use gpui_component::Root;
use image::RgbaImage;

const W: f32 = 320.;
const H: f32 = 330.;

type Events = Rc<RefCell<Vec<ShareEvent>>>;

struct Host {
    share: Entity<SharePopover>,
    open: bool,
}

impl Render for Host {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let this = cx.entity();
        let share = self.share.clone();
        let focus = share.read(cx).focus_handle(cx);
        div().size_full().p(px(16.)).child(
            Popover::new("share", "Share")
                .width(SHARE_POPOVER_WIDTH)
                .open(self.open)
                .settled()
                .focus(&focus)
                .on_open_change(move |open, _, cx| {
                    this.update(cx, |host, cx| {
                        host.open = open;
                        cx.notify();
                    })
                })
                .trigger(
                    EditorButton::new("share-trigger")
                        .label("Share")
                        .icon(IconName::CloudUpload)
                        .kind(ButtonKind::Plain),
                )
                .content(move |_, _| share.clone()),
        )
    }
}

struct Fixture {
    app: App,
    host: Entity<Host>,
    share: Entity<SharePopover>,
    events: Events,
}

impl Fixture {
    fn open(
        mode: ThemeMode,
        setup: impl FnOnce(&mut SharePopover, &mut Context<SharePopover>),
    ) -> Self {
        let events = Events::default();
        let heard = events.clone();
        let mut parts = None;
        let mut app = Headless::open(Assets, es_ui::init, W, H, |window, cx| {
            es_ui::theme::set_mode(mode, cx);
            let share = cx.new(|cx| {
                let mut share = SharePopover::new(window, cx);
                setup(&mut share, cx);
                share
            });
            let host = cx.new(|cx| {
                cx.subscribe(&share, move |_, _, event: &ShareEvent, _| {
                    heard.borrow_mut().push(event.clone())
                })
                .detach();
                Host {
                    share: share.clone(),
                    open: true,
                }
            });
            parts = Some((host.clone(), share));
            cx.new(|cx| Root::new(host, window, cx))
        });
        support::activate(&mut app);
        let (host, share) = parts.expect("the window built its host");
        let mut fixture = Self {
            app,
            host,
            share,
            events,
        };
        fixture.focus_popover();
        fixture
    }

    fn focus_popover(&mut self) {
        support::focus(&mut self.app, &self.share);
    }

    fn update(&mut self, change: impl FnOnce(&mut SharePopover, &mut Context<SharePopover>)) {
        self.app.cx.update(|cx| self.share.update(cx, change));
        self.app.settle();
    }

    fn take(&self) -> Vec<ShareEvent> {
        std::mem::take(&mut *self.events.borrow_mut())
    }

    fn is_open(&mut self) -> bool {
        self.app.cx.update(|cx| self.host.read(cx).open)
    }

    fn colours(&mut self) -> Colours {
        self.app.cx.update(|cx| {
            let t = tokens(cx);
            Colours {
                accent: t.accent.into(),
                surface: t.dropdown.into(),
                destructive: t.destructive.into(),
            }
        })
    }
}

struct Colours {
    accent: Rgba,
    surface: Rgba,
    destructive: Rgba,
}

fn near(pixel: &image::Rgba<u8>, colour: Rgba) -> bool {
    let want = [colour.r, colour.g, colour.b].map(|c| (c * 255.).round() as i16);
    pixel.0[..3]
        .iter()
        .zip(want)
        .all(|(&got, want)| (i16::from(got) - want).abs() <= 6)
}

fn count(image: &RgbaImage, colour: Rgba) -> usize {
    image.pixels().filter(|p| near(p, colour)).count()
}

/// The centre, in window points, of the pixels drawn in `colour`.
fn centre_of(image: &RgbaImage, colour: Rgba) -> (f32, f32) {
    let scale = image.width() as f32 / W;
    let (mut n, mut x, mut y) = (0., 0., 0.);
    for (px, py, p) in image.enumerate_pixels() {
        if near(p, colour) {
            n += 1.;
            x += px as f32;
            y += py as f32;
        }
    }
    assert!(n > 0., "nothing is drawn in {colour:?}");
    (x / n / scale, y / n / scale)
}

/// A full press and release: a focused control activates on the release.
fn press_key(app: &mut App, keystroke: &str) {
    key(app, keystroke);
    support::send(
        app,
        PlatformInput::KeyUp(KeyUpEvent {
            keystroke: support::stroke(keystroke),
        }),
    );
}

fn video(share: &mut SharePopover, cx: &mut Context<SharePopover>) {
    share.set_offer_days(false, cx);
    share.set_info(Some("1080p, about 38 MB".into()), cx);
}

const VIEWS: [&str; 6] = [
    "ready",
    "video-bucket",
    "busy",
    "too-long",
    "failed",
    "failed-wait",
];

fn set_up(view: &str, share: &mut SharePopover, cx: &mut Context<SharePopover>) {
    match view {
        "ready" => share.set_days(3, cx),
        "video-bucket" => video(share, cx),
        "busy" => share.set_busy(true, cx),
        "too-long" => share.set_view(
            ShareView::TooLong {
                message: "This video is 31 min. easyshot Free takes about 27 min at 720p.".into(),
            },
            cx,
        ),
        "failed" => share.set_view(
            ShareView::Failed {
                message: "The upload stopped. Check your connection.".into(),
                wait: None,
            },
            cx,
        ),
        "failed-wait" => share.set_view(
            ShareView::Failed {
                message: "You have shared a lot today.".into(),
                wait: Some("Try again in 12 min".into()),
            },
            cx,
        ),
        _ => unreachable!("{view} is not a view"),
    }
}

#[test]
fn every_view_draws_on_the_popover_surface_in_light_and_dark() {
    for (mode, tag) in [(ThemeMode::Light, "light"), (ThemeMode::Dark, "dark")] {
        let mut drawn = Vec::new();
        for view in VIEWS {
            let mut f = Fixture::open(mode, |share, cx| set_up(view, share, cx));
            let image = f.app.screenshot(&format!("share-{view}-{tag}"));
            let c = f.colours();
            let scale = image.width() as f32 / W;
            let area = scale * scale;
            assert!(
                count(&image, c.surface) as f32 > 200. * 100. * area,
                "{view} {tag}: the popover surface is drawn"
            );
            match view {
                "ready" | "video-bucket" | "too-long" | "failed" => assert!(
                    count(&image, c.accent) as f32 > 50. * 20. * area,
                    "{view} {tag}: the main action is a solid accent button"
                ),
                "busy" | "failed-wait" => assert!(
                    (count(&image, c.accent) as f32) < 50. * 20. * area,
                    "{view} {tag}: the waiting button is drawn disabled, not solid"
                ),
                _ => {}
            }
            if view.starts_with("failed") {
                assert!(
                    count(&image, c.destructive) as f32 > 8. * area,
                    "{view} {tag}: the error is marked in the error colour"
                );
            }
            drawn.push(image);
        }
        for (i, a) in drawn.iter().enumerate() {
            for (b, name) in drawn.iter().zip(VIEWS).skip(i + 1) {
                assert_ne!(a, b, "{tag}: {} and {name} look the same", VIEWS[i]);
            }
        }
    }
}

#[test]
fn the_days_are_offered_only_where_shares_expire() {
    let mut free = Fixture::open(ThemeMode::Dark, |_, _| {});
    let free = free.app.screenshot("share-days-free");
    let mut bucket = Fixture::open(ThemeMode::Dark, |share, cx| share.set_offer_days(false, cx));
    let bucket = bucket.app.screenshot("share-days-bucket");
    let rows = |image: &RgbaImage, surface: Rgba| {
        let x = image.width() / 2;
        (0..image.height())
            .filter(|&y| near(image.get_pixel(x, y), surface))
            .count()
    };
    let surface = Fixture::open(ThemeMode::Dark, |_, _| {}).colours().surface;
    assert!(
        rows(&free, surface) > rows(&bucket, surface),
        "a bucket's popover is shorter: its shares never expire, so it has no days to pick"
    );
}

#[test]
fn enter_shares_with_the_typed_title() {
    let mut f = Fixture::open(ThemeMode::Dark, |_, _| {});
    let field = f.app.cx.update(|cx| {
        let share = f.share.read(cx);
        share.title(cx)
    });
    assert_eq!(field, None, "the title starts empty");

    key(&mut f.app, "enter");
    assert_eq!(
        f.take(),
        [ShareEvent::Share { title: None }],
        "with nothing typed, Enter shares untitled"
    );

    key(&mut f.app, "tab");
    support::type_text(&mut f.app, "  Launch notes ");
    key(&mut f.app, "enter");
    assert_eq!(
        f.take(),
        [ShareEvent::Share {
            title: Some("Launch notes".into())
        }],
        "Enter in the title field shares with the title, trimmed"
    );
}

#[test]
fn escape_closes() {
    let mut f = Fixture::open(ThemeMode::Dark, |_, _| {});
    key(&mut f.app, "escape");
    assert_eq!(f.take(), [ShareEvent::Close]);
    assert!(!f.is_open(), "the popover around it closes too");
}

#[test]
fn picking_days_emits_the_value_without_adopting_it() {
    let mut f = Fixture::open(ThemeMode::Dark, |share, cx| share.set_days(3, cx));
    key(&mut f.app, "tab");
    key(&mut f.app, "tab");
    key(&mut f.app, "right");
    assert_eq!(
        f.take(),
        [ShareEvent::Days(7)],
        "from the title field, Tab reaches the days, and Right picks the next"
    );
    key(&mut f.app, "right");
    assert_eq!(
        f.take(),
        [ShareEvent::Days(7)],
        "the caller has not answered, so 3 days is still the choice"
    );
    f.update(|share, cx| share.set_days(7, cx));
    key(&mut f.app, "left");
    assert_eq!(f.take(), [ShareEvent::Days(3)]);
}

#[test]
fn tabbing_from_the_title_reaches_days_then_share_with_nothing_between() {
    let mut f = Fixture::open(ThemeMode::Dark, |_, _| {});
    for _ in 0..3 {
        key(&mut f.app, "tab");
    }
    press_key(&mut f.app, "enter");
    assert_eq!(
        f.take(),
        [ShareEvent::Share { title: None }],
        "no destination row or link sits between the days and Share"
    );
}

#[test]
fn a_disabled_share_emits_nothing() {
    let mut f = Fixture::open(ThemeMode::Dark, |_, _| {});
    let accent = f.colours().accent;
    let image = f.app.screenshot("share-click-probe");
    let (x, y) = centre_of(&image, accent);
    click(&mut f.app, x, y);
    assert_eq!(
        f.take(),
        [ShareEvent::Share { title: None }],
        "a click on Share shares"
    );

    f.update(|share, cx| share.set_busy(true, cx));
    f.focus_popover();
    click(&mut f.app, x, y);
    key(&mut f.app, "enter");
    assert_eq!(
        f.take(),
        [],
        "while the file is prepared, neither a click nor Enter shares"
    );

    f.update(|share, cx| {
        share.set_busy(false, cx);
        share.set_view(
            ShareView::Failed {
                message: "Too many shares.".into(),
                wait: Some("Try again in 12 min".into()),
            },
            cx,
        );
    });
    let image = f.app.screenshot("share-wait-probe");
    let (x, y) = centre_of(&image, f.colours().destructive);
    click(&mut f.app, x + 120., y + 40.);
    key(&mut f.app, "enter");
    assert_eq!(f.take(), [], "a Retry that has to wait does nothing");
}

#[test]
fn retry_and_the_long_video_actions_emit_their_events() {
    let mut f = Fixture::open(ThemeMode::Dark, |share, cx| {
        share.set_view(
            ShareView::TooLong {
                message: "Too long.".into(),
            },
            cx,
        )
    });
    let accent = f.colours().accent;
    let image = f.app.screenshot("share-bucket-probe");
    let (x, y) = centre_of(&image, accent);
    click(&mut f.app, x, y);
    assert_eq!(f.take(), [ShareEvent::SetUpBucket]);

    f.update(|share, cx| {
        share.set_view(
            ShareView::Failed {
                message: "The upload stopped.".into(),
                wait: None,
            },
            cx,
        )
    });
    let image = f.app.screenshot("share-retry-probe");
    let (x, y) = centre_of(&image, accent);
    click(&mut f.app, x, y);
    assert_eq!(f.take(), [ShareEvent::Retry]);
}
