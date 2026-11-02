# easyshot website

The marketing site and how-to guide for easyshot, in `docs/site/`. Nuxt 4 with Tailwind CSS
v4, generated as a static site and served by Cloudflare Workers static assets.

## Pages

- `/`: the home page (`app/pages/index.vue`)
- `/how-to`: the step-by-step guide (`app/pages/how-to.vue`)
- `/sitemap.xml` and `/robots.txt` are made at build time from `server/routes/`

The mock-ups (hero, editor, recording bar) are HTML in `app/components/`, so they theme
themselves and need no screenshots. Images come from the app's own assets: the logo from
`branding/`, wallpapers from `assets/wallpapers/` and the practice image from `assets/`.
Icons are the app's Lucide set from `assets/icons/lucide/`.

## Theme

Colours are CSS tokens in `app/assets/css/main.css`, mapped to Tailwind with `@theme inline`
(`bg-bg`, `bg-surface`, `text-fg-2`, `text-accent`, `border-line`, ...). Light is the
default. Dark follows the system unless the visitor picks a theme with the toggle; it is the
deep teal of the logo, not black. The choice is kept in `localStorage` (`es-theme`) and
applied by a small script in `<head>` before the first paint.

## Work on it

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
