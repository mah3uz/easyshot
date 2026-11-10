# easyshot website

The marketing site and how-to guide for easyshot, in `docs/site/`. Nuxt 4 with Tailwind CSS
v4, generated as a static site and served by Cloudflare Workers static assets.

## Pages

- `/`: the home page (`app/pages/index.vue`)
- `/how-to`: the step-by-step guide (`app/pages/how-to.vue`)
- `/sitemap.xml` and `/robots.txt` are made at build time from `server/routes/`

The hero, editor and recording pictures are real screenshots of easyshot; see "Screenshots"
below. The feature tiles are HTML drawings in `app/components/FeatureBento.vue`.

The logo comes from `branding/`, the editor's background swatches from the app's own
wallpapers (`assets/wallpapers/`), and icons from its Lucide set (`assets/icons/lucide/`).

## Screenshots

`public/img/shots/` holds crops of real easyshot captures (2560×1440 at 2× scale, on
Hyprland): the area selection over a web page (`hero`) with the hovered deck card as an inset
(`deck`), the editor (`editor`), and an area recording (`record`) with its control bar as an
inset (`recbar`). Each is WebP at 400, 640, 1080, 1600 and 2200 px wide, up to the crop's own
width (`<name>-<width>.webp`; the two insets only at their own width), listed with their sizes
and alt text in `app/utils/shots.ts` and drawn by `app/components/RealShot.vue` with
`srcset` and fixed width and height. To swap one, crop the new capture, write the same
sizes, and update its size in `shots.ts`.

## Photos

The pictures inside the feature tiles are four Unsplash photos, used under the
[Unsplash licence](https://unsplash.com/license). They are listed with their Unsplash IDs in
`app/utils/photos.ts` and served from `public/img/photos/` as WebP at 400, 640, 1080, 1600
and 2200 px wide (`<name>-<width>.webp`), picked with `srcset`. Each sits on a drawn
landscape, `public/img/landscape.svg`, shown if a file fails to load.

To add or swap one, download it from
`https://images.unsplash.com/photo-<id>?fm=jpg&fit=crop&w=2200&h=1238&q=82`, write the five
WebP sizes with the same names, and add it to `photos.ts`. The licence does not require
credit; photographer names can be added from each photo's Unsplash page.

## Theme

Colours are CSS tokens in `app/assets/css/main.css`, mapped to Tailwind with `@theme inline`
(`bg-bg`, `bg-surface`, `text-fg-2`, `text-accent`, `border-line`, ...). Light is the
default. Dark follows the system unless the visitor picks a theme with the toggle. Dark is
never black: a gradient of the logo's deep teals (`--page-grad`) under three slowly drifting
glows (`--blob-1` to `--blob-3`, teal, ocean blue and a little indigo), drawn by the fixed
`.page-bg` layer in `app.vue`. Cards and panels are translucent with a backdrop blur, so the
gradient shows through them. The choice is kept in `localStorage` (`es-theme`) and
applied by a small script in `<head>` before the first paint.

## Work on it

From the repository root, `just site-dev` runs the dev server (arguments go to `nuxt dev`), `just site-build` builds the static site, and `just site-deploy` builds and deploys it to Cloudflare (run `npx wrangler login` once first). Each installs the packages first if `node_modules` is missing. Or, inside `docs/site`:

```bash
cd docs/site
npm install
npm run dev
```

## Build and deploy

```bash
npm run build      # nuxt generate → .output/public
npm run preview    # serve the build with wrangler
npm run deploy     # build and deploy with wrangler
```

`wrangler.jsonc` deploys `.output/public` as static assets. Set the public URL used in
canonical links, Open Graph tags and the sitemap with `NUXT_PUBLIC_SITE_URL` at build time
(default `https://easyshot.site`):

```bash
NUXT_PUBLIC_SITE_URL=https://example.com npm run deploy
```

## Open Graph image

`public/og.png` (1200×630) is rendered from HTML by `scripts/og.mjs` with Playwright. Run
`npm run og` after changing the logo or the headline.

## Responsive

The layout is checked from 320 px to 1920 px wide: no horizontal scrolling, and no text
pushed off the screen. The navigation folds into a menu below 1024 px. The screenshots scale
with their container.

## Media to replace

The hero, editor and recording are real screenshots. The rest are HTML drawings, and each
could become real footage recorded with easyshot itself, on a real Hyprland session, at 2×
scale. Record with
easyshot's own recorder, export from its trim viewer, and keep each file small: AV1 or
H.264 MP4 with a WebM fallback, no audio, muted autoplay loop, `playsinline`, and a poster
frame (AVIF or WebP) for `prefers-reduced-motion` and slow connections.

| Slot | Component | What to record | Format |
|---|---|---|---|
| Feature grid | `FeatureBento.vue` | One 3–5 s loop or still per tile: area, window highlight, OCR copying text, colour picker, scrolling capture growing, a pin over windows, the countdown, recording, the deck. | 16:9 or 2:1, 1200 px wide, under 400 KB each |
| Smart Redaction | `pages/index.vue` | A screenshot with an e-mail, a token, a card number and an IP address; press the redaction button and watch the pixelate shapes appear. Use made-up values only. | 4:3 loop, 4–6 s |
| Keyboard | `BindSnippet.vue` | Settings › Shortcuts: Copy Snippet for Hyprland, then Check marking a key that is already bound. | still, 2× PNG or WebP |
| Open Graph | `scripts/og.mjs` | Replace the drawn window with a real editor screenshot once one exists. | 1200×630 PNG, under 300 KB |

Rules for the footage: a clean demo user and made-up data (no real e-mail addresses, tokens
or hostnames), a wallpaper from `assets/wallpapers/`, the default light and dark themes,
English locale, and the pointer visible. 