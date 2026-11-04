use std::collections::HashSet;
use std::sync::Arc;

use cosmic_text::{
    Align, Buffer, CacheKey, CacheKeyFlags, FontSystem, Metrics, Shaping, SwashCache, Wrap,
};
use kurbo::{BezPath, Rect, Size};
use swash::zeno::Command;

use crate::fonts::{FontSet, attrs, installed_families, resolve};
use crate::layout::{LineBox, Outline, PlacedGlyph, TextLayout};
use crate::lru::Lru;
use crate::{FontFamily, TextAlign, TextStyle};

pub const MIN_WIDTH: f64 = 16.0;
pub const LINE_HEIGHT_MULTIPLE: f64 = 1.35;
const LAYOUT_CACHE: usize = 512;
const OUTLINE_CACHE: usize = 256;

#[derive(Clone, PartialEq, Eq, Hash)]
struct Key {
    text: String,
    family: FontFamily,
    size: u64,
    bold: bool,
    italic: bool,
    underline: bool,
    align: TextAlign,
    wrap: Option<u64>,
}

impl Key {
    fn new(text: &str, style: &TextStyle, wrap: Option<f64>) -> Self {
        Self {
            text: text.to_owned(),
            family: style.family.clone(),
            size: style.size.to_bits(),
            bold: style.bold,
            italic: style.italic,
            underline: style.underline,
            align: style.align,
            wrap: wrap.map(f64::to_bits),
        }
    }
}

pub struct TextEngine {
    fonts: FontSystem,
    known: HashSet<String>,
    installed: Vec<String>,
    swash: SwashCache,
    layouts: Lru<Key, Arc<TextLayout>>,
    outlines: Lru<Key, Arc<Outline>>,
}

pub fn line_height(style: &TextStyle) -> f64 {
    style.size.max(1.0) * LINE_HEIGHT_MULTIPLE
}

impl TextEngine {
    pub fn new(fonts: FontSet) -> Self {
        let fonts = fonts.into_system();
        let (known, installed) = installed_families(fonts.db());
        Self {
            fonts,
            known,
            installed,
            swash: SwashCache::new(),
            layouts: Lru::new(LAYOUT_CACHE),
            outlines: Lru::new(OUTLINE_CACHE),
        }
    }

    pub fn bundled() -> Self {
        Self::new(FontSet::bundled())
    }

    pub fn installed() -> Self {
        Self::new(FontSet::installed())
    }

    /// Installed families by name, sorted, without the bundled ones.
    pub fn installed_families(&self) -> &[String] {
        &self.installed
    }

    /// False for an installed family this font set lacks, which draws in the default family.
    pub fn has_family(&self, family: &FontFamily) -> bool {
        match family {
            FontFamily::System(name) => self.known.contains(name),
            _ => true,
        }
    }

    pub fn cached_layouts(&self) -> usize {
        self.layouts.len()
    }

    pub fn measure(&mut self, text: &str, style: &TextStyle, wrap: Option<f64>) -> Arc<TextLayout> {
        let key = Key::new(text, style, wrap);
        if let Some(hit) = self.layouts.get(&key) {
            return hit;
        }
        let layout = Arc::new(self.compute(text, style, wrap));
        self.layouts.put(key, layout.clone());
        layout
    }

    pub fn outline(&mut self, layout: &TextLayout) -> Arc<Outline> {
        let key = Key::new(&layout.text, &layout.style, layout.wrap);
        if let Some(hit) = self.outlines.get(&key) {
            return hit;
        }
        let outline = Arc::new(self.build_outline(layout));
        self.outlines.put(key, outline.clone());
        outline
    }

    fn compute(&mut self, text: &str, style: &TextStyle, wrap: Option<f64>) -> TextLayout {
        let size = style.size.max(1.0);
        let lh = line_height(style);
        let mut buffer = Buffer::new(&mut self.fonts, Metrics::new(size as f32, lh as f32));
        let mut b = buffer.borrow_with(&mut self.fonts);
        let attrs = attrs(
            resolve(&style.family, &self.known),
            style.bold,
            style.italic,
        );
        match wrap {
            Some(w) => {
                b.set_wrap(Wrap::WordOrGlyph);
                b.set_size(Some(w.floor().max(MIN_WIDTH) as f32), None);
            }
            None => {
                b.set_wrap(Wrap::None);
                b.set_size(None, None);
            }
        }
        b.set_text(text, &attrs, Shaping::Advanced, Some(Align::Left));
        b.shape_until_scroll(false);

        let mut para_starts = vec![0usize];
        para_starts.extend(text.match_indices('\n').map(|(i, _)| i + 1));

        struct Raw {
            range: std::ops::Range<usize>,
            top: f64,
            baseline: f64,
            width: f64,
            glyphs: Vec<(std::ops::Range<usize>, f64, f64, CacheKey)>,
        }
        let mut raws: Vec<Raw> = Vec::new();
        let mut first_font = None;
        for run in b.layout_runs() {
            let base = para_starts.get(run.line_i).copied().unwrap_or(0);
            let mut start = base;
            let mut end = base;
            let mut glyphs = Vec::with_capacity(run.glyphs.len());
            for (i, g) in run.glyphs.iter().enumerate() {
                if i == 0 {
                    start = base + g.start;
                    end = base + g.end;
                }
                start = start.min(base + g.start);
                end = end.max(base + g.end);
                first_font.get_or_insert((g.font_id, g.font_weight));
                let (key, _, _) = CacheKey::new(
                    g.font_id,
                    g.glyph_id,
                    g.font_size,
                    (0.0, 0.0),
                    g.font_weight,
                    CacheKeyFlags::DISABLE_HINTING,
                );
                glyphs.push((
                    base + g.start..base + g.end,
                    f64::from(g.x + g.x_offset),
                    f64::from(g.w),
                    key,
                ));
            }
            raws.push(Raw {
                range: start..end,
                top: f64::from(run.line_top),
                baseline: f64::from(run.line_y),
                width: f64::from(run.line_w),
                glyphs,
            });
        }
        if raws.is_empty() {
            raws.push(Raw {
                range: 0..0,
                top: 0.0,
                baseline: lh,
                width: 0.0,
                glyphs: Vec::new(),
            });
        }

        let box_w = match wrap {
            Some(w) => w.floor().max(MIN_WIDTH),
            None => {
                let widest = raws.iter().map(|r| r.width).fold(0.0, f64::max);
                (widest.ceil() + 1.0).max(MIN_WIDTH)
            }
        };
        let height = if text.is_empty() {
            lh
        } else {
            (raws.len() as f64 * lh - 1e-6).ceil().max(lh)
        };

        let mut lines = Vec::with_capacity(raws.len());
        let mut glyphs = Vec::new();
        for (li, r) in raws.into_iter().enumerate() {
            let shift = match style.align {
                TextAlign::Start => 0.0,
                TextAlign::Middle => (box_w - r.width) / 2.0,
                TextAlign::End => box_w - r.width,
            };
            for (range, x, advance, key) in r.glyphs {
                glyphs.push(PlacedGlyph {
                    line: li,
                    range,
                    x: x + shift,
                    advance,
                    key,
                });
            }
            lines.push(LineBox {
                range: r.range,
                top: r.top,
                height: lh,
                baseline: r.baseline,
                x: shift,
                width: r.width,
            });
        }

        let underline = if style.underline {
            first_font.and_then(|(id, weight)| {
                let font = self.fonts.get_font(id, weight)?;
                let m = font.as_swash().metrics(&[]).scale(size as f32);
                Some((
                    f64::from(m.underline_offset),
                    f64::from(m.stroke_size).max(1.0),
                ))
            })
        } else {
            None
        };

        TextLayout {
            size: Size::new(box_w, height),
            lines,
            glyphs,
            style: style.clone(),
            text: text.to_owned(),
            wrap,
            underline,
        }
    }

    fn build_outline(&mut self, layout: &TextLayout) -> Outline {
        let mut path = BezPath::new();
        for g in &layout.glyphs {
            let baseline = layout.lines[g.line].baseline;
            let Some(cmds) = self.swash.get_outline_commands(&mut self.fonts, g.key) else {
                continue;
            };
            let pt = |v: swash::zeno::Vector| {
                kurbo::Point::new(g.x + f64::from(v.x), baseline - f64::from(v.y))
            };
            for c in cmds {
                match *c {
                    Command::MoveTo(p) => path.move_to(pt(p)),
                    Command::LineTo(p) => path.line_to(pt(p)),
                    Command::QuadTo(c, p) => path.quad_to(pt(c), pt(p)),
                    Command::CurveTo(c1, c2, p) => path.curve_to(pt(c1), pt(c2), pt(p)),
                    Command::Close => path.close_path(),
                }
            }
        }
        let mut underlines = Vec::new();
        if let Some((offset, thickness)) = layout.underline {
            for l in layout.lines.iter().filter(|l| l.width > 0.0) {
                let top = l.baseline - offset - thickness / 2.0;
                underlines.push(Rect::new(l.x, top, l.x + l.width, top + thickness));
            }
        }
        Outline { path, underlines }
    }
}
