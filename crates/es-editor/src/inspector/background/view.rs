use es_model::{
    BackgroundStyle,
    background::{Alignment, AspectRatio, BackgroundColor, ShadowStyle},
};
use es_render::beautify::gradients::PRESETS;
use es_ui::{
    a11y::{Accessible, Role},
    components::{AlignmentPad, ButtonKind, EditorButton, Segment, Segmented, Tile, TileGrid},
    icons::IconName,
    theme::tokens,
    tokens::{Tokens, Typeset},
};
use gpui::{
    AnyElement, Context, Div, Hsla, InteractiveElement, IntoElement, ObjectFit, ParentElement,
    Render, Rgba, SharedString, StatefulInteractiveElement, Styled, StyledImage, Window, div, img,
    prelude::FluentBuilder, px, white,
};
use gpui_component::{Icon, color_picker::ColorPicker};

use super::wallpaper_cache::{BUILT_IN, Source, Thumb};
use super::{
    BUILT_IN_COLUMNS, BackgroundBody, GROUP_GAP, Library, RECENT_COLUMNS, WALLPAPER_ASPECT,
    WallpaperTab,
};
use crate::strings::fl;

fn colour_name(id: &str) -> String {
    match id {
        "black" => fl!("editor-bg-color-black"),
        "white" => fl!("editor-bg-color-white"),
        "graphite" => fl!("editor-bg-color-graphite"),
        "red" => fl!("editor-bg-color-red"),
        "orange" => fl!("editor-bg-color-orange"),
        "yellow" => fl!("editor-bg-color-yellow"),
        "green" => fl!("editor-bg-color-green"),
        "blue" => fl!("editor-bg-color-blue"),
        "purple" => fl!("editor-bg-color-purple"),
        "blush" => fl!("editor-bg-color-blush"),
        "mint" => fl!("editor-bg-color-mint"),
        "sky" => fl!("editor-bg-color-sky"),
        "lavender" => fl!("editor-bg-color-lavender"),
        "peach" => fl!("editor-bg-color-peach"),
        "sage" => fl!("editor-bg-color-sage"),
        "sand" => fl!("editor-bg-color-sand"),
        _ => fl!("editor-bg-custom"),
    }
}

fn gradient_name(id: &str) -> String {
    match id {
        "soft-blush" => fl!("editor-bg-gradient-blush-lavender"),
        "soft-peach" => fl!("editor-bg-gradient-peach-apricot"),
        "soft-mint" => fl!("editor-bg-gradient-mint"),
        "soft-blue" => fl!("editor-bg-gradient-powder-blue"),
        "soft-butter" => fl!("editor-bg-gradient-butter-yellow"),
        "soft-lilac" => fl!("editor-bg-gradient-lilac-periwinkle"),
        "soft-sage" => fl!("editor-bg-gradient-sage-green"),
        "soft-coral" => fl!("editor-bg-gradient-coral-rose"),
        "soft-aqua" => fl!("editor-bg-gradient-aqua-cyan"),
        _ => fl!("editor-bg-gradient-dusty-mauve"),
    }
}

pub(super) fn hsla(c: &BackgroundColor) -> Hsla {
    Rgba {
        r: c.red as f32,
        g: c.green as f32,
        b: c.blue as f32,
        a: 1.,
    }
    .into()
}

fn file_stem(path: &std::path::Path) -> String {
    path.file_stem()
        .map_or_else(String::new, |s| s.to_string_lossy().into_owned())
}

fn group_label(t: &Tokens, text: String) -> Div {
    div()
        .typeset(t.type_scale.label)
        .text_color(t.text_secondary)
        .child(text)
}

fn group(t: &Tokens, label: String, body: impl IntoElement) -> Div {
    div()
        .flex()
        .flex_col()
        .gap(t.metrics.group_label_spacing)
        .child(group_label(t, label))
        .child(body)
}

fn thumb_tile(t: &Tokens, id: SharedString, label: String, thumb: Thumb) -> Tile {
    let tile = Tile::new(id, label).aspect(WALLPAPER_ASPECT);
    match thumb {
        Thumb::Ready(image) => tile.child(img(image).size_full().object_fit(ObjectFit::Cover)),
        Thumb::Missing => tile.fill(gpui::black()).child(
            div()
                .absolute()
                .inset(px(4.))
                .rounded(px(3.))
                .border(px(1.))
                .border_color(white().opacity(0.16)),
        ),
        Thumb::Loading | Thumb::Unreadable => tile.fill(t.track),
    }
}

impl BackgroundBody {
    fn colour_picker(&self, t: &Tokens, cx: &mut Context<Self>) -> AnyElement {
        let s = &self.settings(cx).style;
        let presets = BackgroundColor::plain_presets();
        let selected = match s {
            BackgroundStyle::Solid(c) => presets.iter().position(|p| p.id == c.id),
            _ => None,
        };
        let tiles = presets
            .iter()
            .map(|c| Tile::new(SharedString::from(c.id.clone()), colour_name(&c.id)).fill(hsla(c)));
        let this = cx.entity().downgrade();
        let grid = TileGrid::new("bg-colors", fl!("editor-bg-fill-color"), tiles)
            .selected(selected)
            .on_select(move |i, _, cx| {
                let _ = this.update(cx, |this, cx| this.pick_color(i, cx));
            });
        let picker = self.controls.as_ref().map(|c| {
            ColorPicker::new(&c.picker)
                .icon(Icon::new(IconName::Pipette).size(px(14.)))
                .accessibility_label(fl!("editor-bg-custom-help"))
        });
        let custom = match s {
            BackgroundStyle::Solid(c) if c.id == "custom" => Some(hsla(c)),
            _ => None,
        };
        div()
            .flex()
            .flex_col()
            .gap(t.metrics.row_spacing)
            .child(grid)
            .child(
                div()
                    .id("bg-custom")
                    .flex()
                    .items_center()
                    .justify_between()
                    .h(px(24.))
                    .child(group_label(t, fl!("editor-bg-custom")))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(8.))
                            .when_some(custom, |d, c| {
                                d.child(div().size(px(18.)).rounded(px(4.)).bg(c))
                            })
                            .children(picker),
                    ),
            )
            .into_any_element()
    }

    fn gradient_picker(&self, cx: &mut Context<Self>) -> AnyElement {
        let selected = match &self.settings(cx).style {
            BackgroundStyle::Gradient(g) => PRESETS.iter().position(|p| p.id == g.id),
            _ => None,
        };
        let tiles = PRESETS.iter().zip(&self.gradients).map(|(p, image)| {
            let tile = Tile::new(p.id, gradient_name(p.id));
            match image {
                Some(image) => tile.child(img(image.clone()).size_full()),
                None => tile,
            }
        });
        let this = cx.entity().downgrade();
        TileGrid::new("bg-gradients", fl!("editor-bg-fill-gradient"), tiles)
            .selected(selected)
            .on_select(move |i, _, cx| {
                let _ = this.update(cx, |this, cx| this.pick_gradient(i, cx));
            })
            .into_any_element()
    }

    fn wallpaper_picker(&mut self, t: &Tokens, cx: &mut Context<Self>) -> AnyElement {
        let this = cx.entity().downgrade();
        let tab = self.tab;
        let tabs = Segmented::new(
            "bg-wallpaper-source",
            fl!("editor-bg-fill-wallpaper"),
            [
                Segment::text(fl!("editor-bg-wallpaper-recent")),
                Segment::text(fl!("editor-bg-wallpaper-system")),
            ],
        )
        .selected(usize::from(tab == WallpaperTab::BuiltIn))
        .on_select({
            let this = this.clone();
            move |i, _, cx| {
                let tab = [WallpaperTab::Recent, WallpaperTab::BuiltIn][i.min(1)];
                let _ = this.update(cx, |this, cx| this.set_wallpaper_tab(tab, cx));
            }
        });
        let grid = match tab {
            WallpaperTab::Recent => self.recent_grid(t, cx),
            WallpaperTab::BuiltIn => self.built_in_grid(t, cx),
        };
        div()
            .flex()
            .flex_col()
            .gap(t.metrics.row_spacing)
            .child(tabs)
            .child(grid)
            .into_any_element()
    }

    fn built_in_grid(&mut self, t: &Tokens, cx: &mut Context<Self>) -> AnyElement {
        let sources = (0..BUILT_IN.len()).map(Source::BuiltIn);
        self.cache
            .update(cx, |cache, cx| cache.request(sources, cx));
        let cache = self.cache.read(cx);
        let tiles: Vec<Tile> = BUILT_IN
            .iter()
            .enumerate()
            .map(|(i, b)| thumb_tile(t, b.id.into(), b.id.into(), cache.get(&Source::BuiltIn(i))))
            .collect();
        let this = cx.entity().downgrade();
        TileGrid::new("bg-built-in", fl!("editor-bg-wallpaper-system"), tiles)
            .columns(BUILT_IN_COLUMNS)
            .spacing(px(7.))
            .selected(self.built_in_index(cx))
            .on_select(move |i, _, cx| {
                let _ = this.update(cx, |this, cx| this.pick_built_in(i, cx));
            })
            .into_any_element()
    }

    fn recent_grid(&mut self, t: &Tokens, cx: &mut Context<Self>) -> AnyElement {
        let shown = self.shown_recent().to_vec();
        let sources = shown.iter().cloned().map(Source::File);
        self.cache
            .update(cx, |cache, cx| cache.request(sources, cx));
        let current = match &self.settings(cx).style {
            BackgroundStyle::Wallpaper { path } => Some(path.clone()),
            _ => None,
        };
        let cache = self.cache.read(cx);
        let this = cx.entity().downgrade();
        let tiles = shown.into_iter().enumerate().map(|(i, path)| {
            let thumb = cache.get(&Source::File(path.clone()));
            let label = match thumb {
                Thumb::Missing => fl!("editor-bg-wallpaper-missing", name = file_stem(&path)),
                _ => file_stem(&path),
            };
            let this = this.clone();
            let picked = path.clone();
            thumb_tile(t, SharedString::from(format!("recent-{i}")), label, thumb)
                .selected(current.as_ref() == Some(&path))
                .on_click(move |_, _, cx| {
                    let path = picked.clone();
                    let _ = this.update(cx, |this, cx| this.pick_recent(path, cx));
                })
                .into_any_element()
        });
        let tiles: Vec<AnyElement> = tiles.collect();
        let add = div()
            .id("bg-wallpaper-add")
            .p(px(2.5))
            .accessible(Role::Button, fl!("editor-bg-wallpaper-add-help"))
            .child(
                div()
                    .w_full()
                    .aspect_ratio(WALLPAPER_ASPECT)
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(t.metrics.tile_radius)
                    .border(px(1.2))
                    .border_dashed()
                    .border_color(t.text_tertiary.opacity(0.6))
                    .text_color(t.text_tertiary)
                    .hover(|s| s.bg(t.hover_fill.opacity(0.4)))
                    .child(Icon::new(IconName::Plus).size(px(13.))),
            )
            .on_click(move |_, _, cx| {
                let _ = this.update(cx, |this, cx| this.choose_wallpaper(cx).detach());
            });
        div()
            .id("bg-recent")
            .w_full()
            .grid()
            .grid_cols(RECENT_COLUMNS)
            .gap(px(6.))
            .accessible(Role::RadioGroup, fl!("editor-bg-wallpaper-recent"))
            .children(tiles)
            .child(add)
            .into_any_element()
    }

    fn layout(&self, t: &Tokens, cx: &mut Context<Self>) -> AnyElement {
        let s = self.settings(cx).clone();
        let this = cx.entity().downgrade();
        let sliders = self.controls.as_ref().map(|c| {
            div()
                .flex()
                .flex_col()
                .gap(t.metrics.row_spacing)
                .child(c.padding.clone())
                .child(c.shadow.clone())
                .child(c.corners.clone())
        });
        let styles = [
            ShadowStyle::Soft,
            ShadowStyle::Long,
            ShadowStyle::Glow,
            ShadowStyle::Crisp,
        ];
        let shadow_style = Segmented::new(
            "bg-shadow-style",
            fl!("editor-bg-shadow-style"),
            [
                Segment::text(fl!("editor-bg-shadow-soft")),
                Segment::text(fl!("editor-bg-shadow-long")),
                Segment::text(fl!("editor-bg-shadow-glow")),
                Segment::text(fl!("editor-bg-shadow-crisp")),
            ],
        )
        .selected(
            styles
                .iter()
                .position(|&x| x == s.shadow_style)
                .unwrap_or(0),
        )
        .on_select({
            let this = this.clone();
            move |i, _, cx| {
                let style = styles[i.min(3)];
                let _ = this.update(cx, |this, cx| this.set_shadow_style(style, cx));
            }
        });
        let shapes = [
            AspectRatio::Auto,
            AspectRatio::Square,
            AspectRatio::FourThree,
            AspectRatio::ThreeTwo,
            AspectRatio::SixteenNine,
        ];
        let shape_on = self.shape_enabled(cx);
        let shape = Segmented::new(
            "bg-canvas-shape",
            fl!("editor-bg-canvas-shape"),
            [
                Segment::text(fl!("editor-bg-shape-free")),
                Segment::text(fl!("editor-aspect-1-1")),
                Segment::text(fl!("editor-aspect-4-3")),
                Segment::text(fl!("editor-aspect-3-2")),
                Segment::text(fl!("editor-aspect-16-9")),
            ],
        )
        .selected(
            shapes
                .iter()
                .position(|&x| x == s.aspect_ratio)
                .unwrap_or(0),
        )
        .disabled(!shape_on)
        .on_select({
            let this = this.clone();
            move |i, _, cx| {
                let aspect = shapes[i.min(4)];
                let _ = this.update(cx, |this, cx| this.set_aspect_ratio(aspect, cx));
            }
        });
        let enabled = self.alignment_enabled(cx);
        let pad = AlignmentPad::new("bg-alignment", fl!("editor-bg-align-pad-a11y"))
            .selected(if enabled {
                s.alignment
            } else {
                Alignment::Center
            })
            .disabled(!enabled)
            .help(if enabled {
                fl!("editor-bg-align-pad-help")
            } else {
                fl!("editor-bg-align-pad-disabled-help")
            })
            .on_select(move |alignment, _, cx| {
                let _ = this.update(cx, |this, cx| this.set_alignment(alignment, cx));
            });
        div()
            .flex()
            .flex_col()
            .gap(GROUP_GAP)
            .child(group(t, fl!("editor-bg-canvas-shape"), shape))
            .child(group(t, fl!("editor-bg-layout"), div().children(sliders)))
            .child(group(
                t,
                fl!("editor-bg-shadow-style"),
                div()
                    .when(s.shadow == 0.0, |d| d.opacity(0.45))
                    .child(shadow_style),
            ))
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(group_label(t, fl!("editor-bg-alignment")))
                    .child(pad),
            )
            .into_any_element()
    }
}

impl Render for BackgroundBody {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.ensure_controls(window, cx);
        let t = tokens(cx).clone();
        let none = self.settings(cx).style == BackgroundStyle::None;
        let this = cx.entity().downgrade();
        let no_background = EditorButton::new("bg-none")
            .kind(ButtonKind::Bordered)
            .icon(if none {
                IconName::Check
            } else {
                IconName::SquareSlash
            })
            .label(fl!("editor-bg-none"))
            .selected(none)
            .on_click({
                let this = this.clone();
                move |_, _, cx| {
                    let _ = this.update(cx, |this, cx| this.pick_none(cx));
                }
            });
        let libraries = [Library::Color, Library::Gradient, Library::Wallpaper];
        let library = Segmented::new(
            "bg-library",
            fl!("editor-bg-fill-library"),
            [
                Segment::text(fl!("editor-bg-fill-color")),
                Segment::text(fl!("editor-bg-fill-gradient")),
                Segment::text(fl!("editor-bg-fill-wallpaper")),
            ],
        )
        .selected(
            libraries
                .iter()
                .position(|&l| l == self.library)
                .unwrap_or(1),
        )
        .on_select(move |i, _, cx| {
            let library = libraries[i.min(2)];
            let _ = this.update(cx, |this, cx| this.set_library(library, cx));
        });
        let picker = match self.library {
            Library::Color => self.colour_picker(&t, cx),
            Library::Gradient => self.gradient_picker(cx),
            Library::Wallpaper => self.wallpaper_picker(&t, cx),
        };
        div()
            .flex()
            .flex_col()
            .gap(GROUP_GAP)
            .child(div().flex().child(no_background))
            .child(group(
                &t,
                fl!("editor-bg-fill-library"),
                div()
                    .flex()
                    .flex_col()
                    .gap(px(12.))
                    .child(library)
                    .child(picker),
            ))
            .child(div().h(t.metrics.hairline).bg(t.separator.opacity(0.45)))
            .child(self.layout(&t, cx))
    }
}
