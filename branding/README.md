# Branding

The logo has exactly two sources:

- `logo.svg`: the full-colour app icon, 512×512 viewBox.
- `logo-symbolic.svg`: the one-colour glyph for panels and trays, 24×24 viewBox, filled `currentColor` paths only (no strokes, no masks), drawn on a 16 px grid so it stays crisp at 16 px.

There are two masters because a colour icon can't be turned into a good one-colour tray glyph automatically.

To change the logo, replace these two files and run `just logo` (needs `rsvg-convert` from librsvg, and Python 3 with fontTools). It rewrites every derived file, which are committed:

- `assets/icons/app/easyshot.svg`: scalable app icon (copy of `logo.svg`).
- `assets/icons/app/hicolor/<N>x<N>/apps/easyshot.png` for N = 16, 22, 24, 32, 48, 64, 128, 256, 512.
- `assets/icons/app/hicolor/symbolic/apps/easyshot-symbolic.svg` (copy of `logo-symbolic.svg`).
- `assets/icons/easyshot.svg`: the tray and in-app icon the UI embeds (copy of `logo-symbolic.svg`).
- `assets/banner/banner-light.svg`, `assets/banner/banner-dark.svg`: the README banner for light and dark pages, built by `scripts/banner.py` with `logo.svg` inlined and the text converted to outlines from the bundled Inter Tight SemiBold and Inter fonts, so it renders the same without them installed.
- `assets/banner/banner-light.png`, `assets/banner/banner-dark.png`: 2× renders of the banners (2560×800).

Never edit the derived files by hand. After changing a master, look at the 16, 22 and 24 px renders: if they turn to mush, simplify the master rather than adding a small-size variant.

`concepts/` keeps every logo concept drawn so far (colour + symbolic). To switch to one, copy it over the two masters and run `just logo`.
