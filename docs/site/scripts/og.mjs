// Renders public/og.png (1200×630), the Open Graph and Twitter card image.
// Needs Playwright with Chromium: `node scripts/og.mjs`.
import { createRequire } from 'node:module'
import { readFileSync } from 'node:fs'
import { fileURLToPath } from 'node:url'

const require = createRequire(import.meta.url)
const { chromium } = require('playwright')
const root = fileURLToPath(new URL('..', import.meta.url))
const logo = readFileSync(`${root}public/logo.svg`, 'utf8')
const wall = readFileSync(`${root}public/img/wall-3.webp`).toString('base64')
const shot = readFileSync(`${root}public/img/practice.webp`).toString('base64')

const html = `<!doctype html><html><head><meta charset="utf-8">
<link href="https://fonts.googleapis.com/css2?family=Inter:wght@500&family=Inter+Tight:wght@700;800&display=swap" rel="stylesheet">
<style>
*{margin:0;box-sizing:border-box}
body{width:1200px;height:630px;overflow:hidden;background:#04212b;font-family:'Inter',sans-serif;color:#e8f7f6;position:relative}
.glow{position:absolute;inset:-200px -100px auto auto;width:900px;height:900px;background:radial-gradient(closest-side,rgba(63,217,196,.28),transparent)}
.grid{position:absolute;inset:0;background-image:linear-gradient(to right,rgba(160,230,225,.07) 1px,transparent 1px),linear-gradient(to bottom,rgba(160,230,225,.07) 1px,transparent 1px);background-size:56px 56px;-webkit-mask-image:linear-gradient(to right,black,transparent 70%)}
.text{position:absolute;left:72px;top:86px;width:560px}
.brand{display:flex;align-items:center;gap:18px;font-family:'Inter Tight';font-weight:700;font-size:40px;letter-spacing:-.02em}
.brand svg{width:76px;height:76px}
h1{font-family:'Inter Tight';font-weight:800;font-size:66px;line-height:1.02;letter-spacing:-.035em;margin-top:44px}
h1 span{background:linear-gradient(120deg,#8ff5e3,#3fd9c4 45%,#5fb8ff);-webkit-background-clip:text;color:transparent}
p{margin-top:26px;font-size:24px;line-height:1.45;color:#a3c3c8}
.pills{display:flex;gap:10px;margin-top:34px}
.pills b{font-weight:500;font-size:18px;padding:7px 16px;border-radius:99px;border:1px solid rgba(160,230,225,.2);color:#cfe9ea}
.frame{position:absolute;left:680px;top:110px;width:620px;height:420px;border-radius:26px;overflow:hidden;border:1px solid rgba(160,230,225,.2);box-shadow:0 50px 100px -30px rgba(0,10,16,.9);background:url(data:image/webp;base64,${wall}) center/cover}
.frame img{position:absolute;left:52px;top:56px;width:460px;border-radius:14px;box-shadow:0 30px 60px -20px rgba(3,30,40,.6)}
.sel{position:absolute;left:84px;top:80px;width:300px;height:200px;border:2px solid #fff;border-radius:3px;box-shadow:0 0 0 999px rgba(3,30,40,.42)}
</style></head><body>
<div class="glow"></div><div class="grid"></div>
<div class="text">
<div class="brand">${logo}easyshot</div>
<h1>Screenshots, done right on <span>Wayland</span>.</h1>
<p>Capture, annotate and record your screen. Free and open source for Linux.</p>
<div class="pills"><b>Hyprland</b><b>sway</b><b>niri</b></div>
</div>
<div class="frame"><img src="data:image/webp;base64,${shot}"><div class="sel"></div></div>
</body></html>`

const browser = await chromium.launch()
const page = await browser.newPage({ viewport: { width: 1200, height: 630 } })
await page.setContent(html, { waitUntil: 'networkidle' })
await page.screenshot({ path: `${root}public/og.png` })
await browser.close()
console.log('wrote public/og.png')
