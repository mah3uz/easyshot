# Branding

The logo has exactly two sources:

- `logo.svg`: the full-colour app icon, 512×512 viewBox.
- `logo-symbolic.svg`: the one-colour glyph for panels and trays, 24×24 viewBox, filled `currentColor` paths only (no strokes, no masks), drawn on a 16 px grid so it stays crisp at 16 px.

There are two masters because a colour icon can't be turned into a good one-colour tray glyph automatically.

To change the logo, replace these two files and run `just logo` (needs `rsvg-convert` from librsvg). It rewrites every derived file, which are committed:

- `assets/icons/app/dev.easyshot.Easyshot.svg`: scalable app icon (copy of `logo.svg`).
- `assets/icons/app/hicolor/<N>x<N>/apps/dev.easyshot.Easyshot.png` for N = 16, 22, 24, 32, 48, 64, 128, 256, 512.
- `assets/icons/app/hicolor/symbolic/apps/dev.easyshot.Easyshot-symbolic.svg` (copy of `logo-symbolic.svg`).
- `assets/icons/easyshot.svg`: the tray and in-app icon the UI embeds (copy of `logo-symbolic.svg`).

Never edit the derived files by hand. After changing a master, look at the 16, 22 and 24 px renders: if they turn to mush, simplify the master rather than adding a small-size variant.

`concepts/` keeps every logo concept drawn so far (colour + symbolic). To switch to one, copy it over the two masters and run `just logo`.
