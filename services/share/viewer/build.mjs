import { readFileSync, writeFileSync } from 'node:fs'
import { fileURLToPath } from 'node:url'

const here = (p) => fileURLToPath(new URL(p, import.meta.url))
const read = (p) => readFileSync(here(p), 'utf8')

const site = read('../../../docs/site/app/assets/css/main.css')

function block(css, prelude) {
  const start = css.indexOf(`\n${prelude} {`)
  if (start < 0) throw new Error(`main.css has no "${prelude}" rule`)
  let depth = 0
  for (let i = css.indexOf('{', start); i < css.length; i++) {
    if (css[i] === '{') depth++
    else if (css[i] === '}' && --depth === 0) return css.slice(start + 1, i + 1)
  }
  throw new Error(`"${prelude}" in main.css is not closed`)
}

function declaration(css, name) {
  const m = css.match(new RegExp(`\\n\\s*(--${name}:[^;]+;)`))
  if (!m) throw new Error(`main.css has no --${name}`)
  return m[1]
}

const fromSite = [
  block(site, ':root'),
  block(site, '@media (prefers-color-scheme: dark)'),
  block(site, ":root[data-theme='dark']"),
  `:root {\n  ${['font-sans', 'font-display', 'ease-out-soft', 'ease-hover', 'radius-card'].map((n) => declaration(site, n)).join('\n  ')}\n}`,
  ...['.page-bg', '.page-bg i', '.page-bg .b1', '.page-bg .b2', '.page-bg .b3', '@keyframes drift1', '@keyframes drift2'].map((p) => block(site, p)),
].join('\n')

const face = (family, weight, file) =>
  `@font-face {\nfont-family: '${family}';\nfont-weight: ${weight};\nfont-display: swap;\nsrc: url(data:font/woff2;base64,${readFileSync(here(`src/fonts/${file}`)).toString('base64')}) format('woff2');\n}`
const fonts = [face('Inter', '100 500', 'Inter-Regular.woff2'), face('Inter', '600 900', 'Inter-Bold.woff2'), face('Inter Tight', '100 900', 'InterTight-Bold.woff2')].join('\n')

const stripComments = (css) => css.replace(/\/\*[\s\S]*?\*\//g, '')
const tidy = (text) =>
  text
    .split('\n')
    .map((l) => l.trim())
    .filter(Boolean)
    .join('\n')

const css = `${fonts}\n${tidy(stripComments(`${fromSite}\n${read('src/viewer.css')}`))}`
const js = tidy(read('src/viewer.js').replace(/^\s*\/\/.*$/gm, ''))

const logo = read('../../../branding/logo.svg')
const symbol = logo.replace(/^<svg[^>]*>/, '<symbol id="logo" viewBox="0 0 512 512">').replace(/<\/svg>\s*$/, '</symbol>')
const favicon = `data:image/svg+xml,${encodeURIComponent(logo.trim())}`

const parts = { css: `<style>\n${css}\n</style>`, js: `<script>\n${js}\n</script>`, logo: symbol, favicon }
let page = read('src/page.html').replace(/@@([a-z]+)@@/g, (_, name) => {
  if (!(name in parts)) throw new Error(`page.html names an unknown part @@${name}@@`)
  return parts[name]
})

for (const inner of page.matchAll(/<(script|style)\b[^>]*>([\s\S]*?)<\/\1>/g)) {
  if (/\{\{|<!--(if|end):/.test(inner[2])) throw new Error(`a placeholder or block marker is inside <${inner[1]}>`)
}
for (const name of ['ready', 'uploading', 'missing', 'image', 'video', 'expires']) {
  if (!page.includes(`<!--if:${name}-->`)) throw new Error(`block ${name} is missing`)
}
for (const name of ['title', 'page_url', 'media_url', 'poster_url', 'mime', 'width', 'height', 'expires_at', 'download_name', 'home_url']) {
  if (!page.includes(`{{${name}}}`)) throw new Error(`placeholder ${name} is missing`)
}
const opened = [...page.matchAll(/<!--if:([a-z]+)-->/g)].length
const closed = [...page.matchAll(/<!--end:([a-z]+)-->/g)].length
if (opened !== closed) throw new Error('block markers are unbalanced')

page = page.replace(/\n{2,}/g, '\n')
writeFileSync(here('viewer.html'), page)
console.log(`viewer.html: ${page.length} bytes`)
