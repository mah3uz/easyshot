# easyshot website

The marketing site and how-to guide for easyshot, in `docs/site/`. Nuxt 4 with Tailwind CSS
v4, generated as a static site and served by Cloudflare Workers static assets.

## Pages

- `/`: the home page (`app/pages/index.vue`)
- `/how-to`: the step-by-step guide (`app/pages/how-to.vue`)
- `/sitemap.xml` and `/robots.txt` are made at build time from `server/routes/`

The mock-ups (hero, editor, recording bar) are HTML in `app/components/`, so they theme
themselves. They stand in for real footage of the app; see "Media to replace" below.

The logo comes from `branding/`, the editor's background swatches from the app's own
wallpapers (`assets/wallpapers/`), and icons from its Lucide set (`assets/icons/lucide/`).

## Photos

The pictures inside the mock-ups are six Unsplash photos, used under the
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
(default `https://easyshot.dev`):

```bash
NUXT_PUBLIC_SITE_URL=https://example.com npm run deploy
```

## Open Graph image

`public/og.png` (1200×630) is rendered from HTML by `scripts/og.mjs` with Playwright. Run
`npm run og` after changing the logo or the headline.

## Responsive

The layout is checked from 320 px to 1920 px wide: no horizontal scrolling, and no text
pushed off the screen. The navigation folds into a menu below 1024 px. The mock-ups scale
with their container (container query units in `ShotWindow.vue`).

## Media to replace

Every picture of the app on the site is an HTML mock-up. Each should become real footage
recorded with easyshot itself, on a real Hyprland session, at 2× scale. Record with
easyshot's own recorder, export from its trim viewer, and keep each file small: AV1 or
H.264 MP4 with a WebM fallback, no audio, muted autoplay loop, `playsinline`, and a poster
frame (AVIF or WebP) for `prefers-reduced-motion` and slow connections.

| Slot | Component | What to record | Format |
|---|---|---|---|
| Hero | `HeroShot.vue` | A full desktop with a browser and a terminal. Press Super+Shift+4, drag an area over the browser, adjust a handle, release; the card slides into the deck, hover it to show Copy and Save. | 16:9 loop, 8–12 s, 2560×1440, under 2.5 MB, plus a poster |
| Feature grid | `FeatureBento.vue` | One 3–5 s loop or still per tile: area, window highlight, OCR copying text, colour picker, scrolling capture growing, a pin over windows, the countdown, recording, the deck. | 16:9 or 2:1, 1200 px wide, under 400 KB each |
| Editor | `EditorMock.vue` | Open a capture in the editor; draw an arrow and a box, add a step number, run Smart Redaction, then change background, padding, corners and camera 3D in the inspector. Keep the interactive mock-up as a fallback, or show the video above it. | 16:10 loop, 12–15 s, 2560×1600 |
| Smart Redaction | `pages/index.vue` | A screenshot with an e-mail, a token, a card number and an IP address; press the redaction button and watch the pixelate shapes appear. Use made-up values only. | 4:3 loop, 4–6 s |
| Recording | `RecordMock.vue` | Start an area recording: countdown, the highlighted area, the control bar with the clock running, pause and resume, stop, then the trim viewer opening. | 16:10 loop, 8–10 s |
| Keyboard | `BindSnippet.vue` | Settings › Shortcuts: Copy Snippet for Hyprland, then Check marking a key that is already bound. | still, 2× PNG or WebP |
| Open Graph | `scripts/og.mjs` | Replace the drawn window with a real editor screenshot once one exists. | 1200×630 PNG, under 300 KB |

Rules for the footage: a clean demo user and made-up data (no real e-mail addresses, tokens
or hostnames), a wallpaper from `assets/wallpapers/`, the default light and dark themes,
English locale, and the pointer visible. A dark-mode version of the hero is worth recording
too, chosen with `<picture>` or a `prefers-color-scheme` media query.
