import { mkdirSync, readFileSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join, resolve } from 'node:path'
import { fileURLToPath, pathToFileURL } from 'node:url'

const usage = 'node preview.mjs [--out DIR] [--image FILE --image-size WxH] [--video FILE --poster FILE --video-size WxH] [--tall FILE --tall-size WxH]'
const args = Object.fromEntries(
  process.argv.slice(2).reduce((pairs, a, i, all) => (a.startsWith('--') ? [...pairs, [a.slice(2), all[i + 1]]] : pairs), []),
)
if ('help' in args) {
  console.log(usage)
  process.exit(0)
}

const here = (p) => fileURLToPath(new URL(p, import.meta.url))
const template = readFileSync(here('viewer.html'), 'utf8')
const out = resolve(args.out ?? join(tmpdir(), 'easyshot-viewer-preview'))
const media = (path) => pathToFileURL(resolve(path)).href
const size = (wxh) => {
  const [width, height] = wxh.split('x')
  return { width, height }
}

const escape = (v) => String(v).replace(/[&<>"']/g, (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' })[c])

// Same rules as the Worker's filler, except that preview media may be file:// or http://localhost.
function fill(values, blocks) {
  let kept = template
  for (let prev; prev !== kept; ) {
    prev = kept
    kept = kept.replace(/<!--if:([a-z]+)-->([\s\S]*?)<!--end:\1-->/, (_, name, body) => (blocks.includes(name) ? body : ''))
  }
  return kept.replace(/\{\{([a-z_]+)\}\}/g, (_, name) => {
    if (!(name in values)) throw new Error(`missing_value: ${name}`)
    const v = values[name]
    if (name.endsWith('_url') && !/^(https:\/\/|file:\/\/|http:\/\/localhost[:/])/.test(v)) throw new Error(`not_https: ${name}`)
    return escape(v)
  })
}

const inThreeDays = new Date(Date.now() + 3 * 864e5 - 6e4).toISOString().replace(/\.\d+Z$/, 'Z')
const base = { page_url: 'https://easyshot.site/s/7Kp2mX9qR4tY8wZ1aB3cD5', home_url: 'https://easyshot.site' }
const image = args.image ?? here('../../../docs/site/public/img/photos/lake-1600.webp')
const states = {
  'ready-image': [
    { ...base, title: 'Login form, wrong error colour', media_url: media(image), poster_url: media(image), mime: 'image/webp', ...size(args['image-size'] ?? '1600x900'), expires_at: inThreeDays, download_name: 'easyshot-7Kp2mX9qR4tY8wZ1aB3cD5.webp' },
    ['ready', 'image', 'expires'],
  ],
  uploading: [{ ...base, title: 'Login form, wrong error colour' }, ['uploading']],
  missing: [{ ...base, title: 'Page not found' }, ['missing']],
}
if (args.video) {
  states['ready-video'] = [
    { ...base, title: 'Recording 2026-11-11 10:42', media_url: media(args.video), poster_url: media(args.poster ?? image), mime: 'video/mp4', ...size(args['video-size'] ?? '1280x720'), expires_at: new Date(Date.now() + 5 * 36e5).toISOString(), download_name: 'easyshot-9Qw3eR5tY7uI.mp4' },
    ['ready', 'video', 'expires'],
  ]
}
if (args.tall) {
  states['ready-tall'] = [
    { ...base, title: 'A very long title that someone typed in full because they wanted to explain exactly which settings page shows the wrong colour on the login form', media_url: media(args.tall), poster_url: media(args.tall), mime: 'image/png', ...size(args['tall-size'] ?? '900x3200'), download_name: 'easyshot-Lm4nB6vC8xZ2aS4dF6gH8j.png' },
    ['ready', 'image'],
  ]
}

mkdirSync(out, { recursive: true })
for (const [name, [values, blocks]] of Object.entries(states)) {
  writeFileSync(join(out, `${name}.html`), fill(values, blocks))
  console.log(join(out, `${name}.html`))
}
