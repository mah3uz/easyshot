#!/usr/bin/env python3
import re
from pathlib import Path

from fontTools.pens.boundsPen import BoundsPen
from fontTools.pens.svgPathPen import SVGPathPen
from fontTools.pens.transformPen import TransformPen
from fontTools.ttLib import TTFont

ROOT = Path(__file__).resolve().parent.parent
LOGO = ROOT / "branding/logo.svg"
FONTS = ROOT / "assets/fonts"
OUT = ROOT / "assets/banner"

W, H = 1280, 400
RADIUS = 28
TILE = 216
GAP = 52
WORDMARK = ("InterTight-SemiBold.ttf", 132, -0.02, "easyshot")
TAGLINE = ("Inter-Regular.ttf", 30, -0.005, "Screenshots, annotation and recording for Linux")
LINE_GAP = 0.40

THEMES = {
    "light": {
        "bg": ("#F3FCFA", "#DDF4F0"),
        "glow": ("#62EDD4", 0.38),
        "border": ("#1388A6", 0.14),
        "bracket": ("#21BDB0", 0.55),
        "back": ("#167A8E", "#0F5F78"),
        "wordmark": "#043040",
        "tagline": "#0F5F78",
    },
    "dark": {
        "bg": ("#0A1A20", "#0D2A31"),
        "glow": ("#21BDB0", 0.24),
        "border": ("#62EDD4", 0.14),
        "bracket": ("#21BDB0", 0.6),
        "back": ("#62EDD4", "#21BDB0"),
        "wordmark": "#FFFFFF",
        "tagline": "#A6D3D6",
    },
}


def num(v):
    s = f"{v:.2f}".rstrip("0").rstrip(".")
    return "0" if s == "-0" else s


class Face:
    def __init__(self, file):
        self.font = TTFont(FONTS / file)
        self.upem = self.font["head"].unitsPerEm
        self.cmap = self.font.getBestCmap()
        self.glyphs = self.font.getGlyphSet()
        self.hmtx = self.font["hmtx"]
        self.pairs = self._pair_subtables()

    def _pair_subtables(self):
        gpos = self.font["GPOS"].table
        out = []
        for rec in gpos.FeatureList.FeatureRecord:
            if rec.FeatureTag != "kern":
                continue
            for i in rec.Feature.LookupListIndex:
                for st in gpos.LookupList.Lookup[i].SubTable:
                    st = getattr(st, "ExtSubTable", st)
                    if st.LookupType == 2:
                        out.append(st)
        return out

    def kern(self, left, right):
        for st in self.pairs:
            if left not in st.Coverage.glyphs:
                continue
            if st.Format == 1:
                pairs = st.PairSet[st.Coverage.glyphs.index(left)].PairValueRecord
                for rec in pairs:
                    if rec.SecondGlyph == right:
                        return getattr(rec.Value1, "XAdvance", 0) or 0
            else:
                c1 = st.ClassDef1.classDefs.get(left, 0)
                c2 = st.ClassDef2.classDefs.get(right, 0)
                return getattr(st.Class1Record[c1].Class2Record[c2].Value1, "XAdvance", 0) or 0
        return 0

    def run(self, text, size, tracking):
        names = [self.cmap[ord(c)] for c in text]
        units = []
        x = 0
        for i, name in enumerate(names):
            units.append((name, x))
            x += self.hmtx[name][0] + tracking * self.upem
            if i + 1 < len(names):
                x += self.kern(name, names[i + 1])
        x -= tracking * self.upem
        scale = size / self.upem
        bounds = BoundsPen(self.glyphs)
        for name, gx in units:
            self.glyphs[name].draw(TransformPen(bounds, (1, 0, 0, 1, gx, 0)))
        xmin, ymin, xmax, ymax = bounds.bounds
        return Run(self, units, scale, x * scale, xmin * scale, ymin * scale, xmax * scale, ymax * scale)


class Run:
    def __init__(self, face, units, scale, advance, xmin, ymin, xmax, ymax):
        self.face, self.units, self.scale = face, units, scale
        self.advance, self.xmin, self.ymin, self.xmax, self.ymax = advance, xmin, ymin, xmax, ymax

    def path(self, x, baseline):
        pen = SVGPathPen(self.face.glyphs, ntos=num)
        s = self.scale
        for name, gx in self.units:
            self.face.glyphs[name].draw(TransformPen(pen, (s, 0, 0, -s, x + gx * s, baseline)))
        return pen.getCommands()


def inline_logo(x, y, size):
    svg = LOGO.read_text()
    view = re.search(r'<svg\b[^>]*\bviewBox="([^"]+)"', svg).group(1)
    inner = re.sub(r"^.*?<svg\b[^>]*>|</svg>\s*$", "", svg, flags=re.S)
    inner = re.sub(r'\bid="([^"]+)"', r'id="logo-\1"', inner)
    inner = re.sub(r'(url\(#|href="#)([^)"]+)', r"\1logo-\2", inner)
    return (
        f'<svg x="{num(x)}" y="{num(y)}" width="{num(size)}" height="{num(size)}" '
        f'viewBox="{view}" overflow="visible">{inner}</svg>'
    )


def bracket(cx, cy, dx, dy, arm=46, r=14):
    return (
        f"M{num(cx)} {num(cy + dy * arm)}V{num(cy + dy * r)}"
        f"Q{num(cx)} {num(cy)} {num(cx + dx * r)} {num(cy)}H{num(cx + dx * arm)}"
    )


def peel(p):
    cx, cy = W, H
    return (
        f"M{num(cx)} {num(cy - p)}"
        f"C{num(cx - 0.34 * p)} {num(cy - 1.03 * p)} {num(cx - 0.68 * p)} {num(cy - 0.94 * p)} "
        f"{num(cx - 0.8 * p)} {num(cy - 0.8 * p)}"
        f"C{num(cx - 0.94 * p)} {num(cy - 0.68 * p)} {num(cx - 1.03 * p)} {num(cy - 0.34 * p)} "
        f"{num(cx - p)} {num(cy)}"
        f"Q{num(cx - 0.56 * p)} {num(cy - 0.56 * p)} {num(cx)} {num(cy - p)}Z"
    )


def banner(theme, word, tag, layout):
    t = THEMES[theme]
    lx, ly, lsize, wx, wy, tx, ty, gx, gy = layout
    p = 104
    inset = 30
    card = f'<rect width="{W}" height="{H}" rx="{RADIUS}"'
    cut = f"M0 0H{W}V{H - p}Q{num(W - 0.56 * p)} {num(H - 0.56 * p)} {W - p} {H}H0Z"
    brackets = "".join(
        bracket(x, y, dx, dy)
        for x, y, dx, dy in ((inset, inset, 1, 1), (W - inset, inset, -1, 1), (inset, H - inset, 1, -1))
    )
    return f"""<svg xmlns="http://www.w3.org/2000/svg" width="{W}" height="{H}" viewBox="0 0 {W} {H}">
<defs>
<linearGradient id="bg" x1="0" y1="0" x2="{W}" y2="{H}" gradientUnits="userSpaceOnUse"><stop offset="0" stop-color="{t["bg"][0]}"/><stop offset="1" stop-color="{t["bg"][1]}"/></linearGradient>
<radialGradient id="glow" cx="{num(gx)}" cy="{num(gy)}" r="{num(lsize * 1.25)}" gradientUnits="userSpaceOnUse"><stop offset="0" stop-color="{t["glow"][0]}" stop-opacity="{t["glow"][1]}"/><stop offset="1" stop-color="{t["glow"][0]}" stop-opacity="0"/></radialGradient>
<linearGradient id="back" x1="0" y1="{H - p}" x2="0" y2="{H}" gradientUnits="userSpaceOnUse"><stop offset="0" stop-color="{t["back"][0]}"/><stop offset="1" stop-color="{t["back"][1]}"/></linearGradient>
<linearGradient id="flap" x1="{num(W - 0.1 * p)}" y1="{num(H - 0.1 * p)}" x2="{num(W - 0.8 * p)}" y2="{num(H - 0.8 * p)}" gradientUnits="userSpaceOnUse"><stop offset="0" stop-color="#A6D3D6"/><stop offset="0.5" stop-color="#E9F7F7"/><stop offset="1" stop-color="#FFFFFF"/></linearGradient>
<filter id="flapShadow" x="-30%" y="-30%" width="160%" height="160%"><feGaussianBlur stdDeviation="5"/></filter>
<clipPath id="cardClip">{card}/></clipPath>
<clipPath id="cutClip"><path d="{cut}"/></clipPath>
</defs>
<g clip-path="url(#cardClip)">
{card} fill="url(#back)"/>
<g clip-path="url(#cutClip)">
<rect width="{W}" height="{H}" fill="url(#bg)"/>
<rect width="{W}" height="{H}" fill="url(#glow)"/>
<path d="{peel(p)}" transform="translate(-5 6)" fill="#043040" fill-opacity="0.28" filter="url(#flapShadow)"/>
</g>
<path d="{peel(p)}" fill="url(#flap)"/>
</g>
<rect x="0.5" y="0.5" width="{W - 1}" height="{H - 1}" rx="{RADIUS - 0.5}" fill="none" stroke="{t["border"][0]}" stroke-opacity="{t["border"][1]}" clip-path="url(#cutClip)"/>
<path d="{brackets}" fill="none" stroke="{t["bracket"][0]}" stroke-opacity="{t["bracket"][1]}" stroke-width="5" stroke-linecap="round"/>
{inline_logo(lx, ly, lsize)}
<path d="{word.path(wx, wy)}" fill="{t["wordmark"]}"/>
<path d="{tag.path(tx, ty)}" fill="{t["tagline"]}"/>
</svg>
"""


def layout(word, tag):
    lsize = TILE * 512 / 448
    text_w = max(word.xmax - word.xmin, tag.xmax - tag.xmin)
    left = (W - (TILE + GAP + text_w)) / 2
    top = word.ymax
    gap = WORDMARK[1] * LINE_GAP
    block = top + gap + tag.ymax
    wy = H / 2 - block / 2 + top
    ty = wy + gap + tag.ymax
    text_x = left + TILE + GAP
    lx = left - (lsize - TILE) / 2
    ly = H / 2 - lsize / 2
    return lx, ly, lsize, text_x - word.xmin, wy, text_x - tag.xmin, ty, left + TILE / 2, H / 2


def main():
    file, size, tracking, text = WORDMARK
    word = Face(file).run(text, size, tracking)
    file, size, tracking, text = TAGLINE
    tag = Face(file).run(text, size, tracking)
    geo = layout(word, tag)
    OUT.mkdir(parents=True, exist_ok=True)
    for theme in THEMES:
        (OUT / f"banner-{theme}.svg").write_text(banner(theme, word, tag, geo))


if __name__ == "__main__":
    main()
