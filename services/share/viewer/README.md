# Share page

`viewer.html` is built; edit `src/` and rebuild. Both scripts are plain Node with no packages.

- `node build.mjs` writes `viewer.html` from `src/page.html`, `src/viewer.css` and `src/viewer.js`.
  It copies the colour tokens, font stacks, drifting backdrop and scrollbar rules from
  `docs/site/app/assets/css/main.css`, inlines `branding/logo.svg` and the fonts in `src/fonts/`,
  and fails if a placeholder or block marker lands inside `<script>` or `<style>`, or if a block or
  placeholder of `PROTOCOL.md` is missing. Rebuild after the site's tokens change.
- `node preview.mjs --out DIR [--video FILE --poster FILE] [--tall FILE --tall-size WxH]` fills
  the page for each state (`ready-image`, `ready-video`, `uploading`, `missing`, `ready-tall`) with
  sample values. It follows the filling rules of `PROTOCOL.md`, except that media may be `file://`
  or `http://localhost`. `--help` lists every option.

The fonts are Latin subsets of `assets/fonts/` (Inter Regular and Bold, Inter Tight Bold), made with:

```sh
pyftsubset assets/fonts/<name>.ttf --unicodes="U+0020-007E,U+00A0-00FF,U+2013-2014,U+2018-201D,U+2026,U+00D7,U+2022" \
  --layout-features="kern,liga,calt,tnum" --flavor=woff2 --no-hinting --desubroutinize \
  --output-file=services/share/viewer/src/fonts/<name>.woff2
```
