<script setup lang="ts">
const { siteUrl, repo } = useRuntimeConfig().public

usePageSeo({
  title: 'How to take screenshots and record your screen on Wayland',
  description:
    'Install easyshot, bind your keys on Hyprland, sway or niri, and take, annotate and record your first capture. A step-by-step guide.',
  path: '/how-to',
})

const steps = [
  { id: 'requirements', title: 'Check what you need' },
  { id: 'install', title: 'Install easyshot' },
  { id: 'start', title: 'Start the service' },
  { id: 'shortcuts', title: 'Bind your keys' },
  { id: 'capture', title: 'Take a screenshot' },
  { id: 'annotate', title: 'Annotate and share' },
  { id: 'record', title: 'Record your screen' },
  { id: 'settings', title: 'Settings and files' },
]

const needs = [
  { icon: 'monitor', title: 'A Wayland compositor', text: 'Hyprland 0.57+, sway 1.12 or niri 26.04. Others with layer-shell and a capture protocol work for basic capture. Not KDE or GNOME.' },
  { icon: 'scan-text', title: 'Tesseract (optional)', text: 'For text capture (OCR) and Smart Redaction. Install the English data too, for example tesseract-data-eng.' },
  { icon: 'video', title: 'A recorder (optional)', text: 'gpu-screen-recorder (preferred) or wf-recorder for recording; ffmpeg for trimming and export.' },
]

const keys = [
  ['Capture an area', 'capture region'],
  ['Capture the whole screen', 'capture fullscreen'],
  ['Capture the previous area, window or display again', 'capture previous'],
  ['Open or close the capture bar', 'capture-bar'],
  ['Record the screen', 'record'],
  ['Capture text (OCR)', 'ocr'],
  ['Pick a colour', 'color-picker'],
]

const captures = [
  { icon: 'square-dashed', title: 'Area', cmd: 'easyshot capture region', text: 'Drag a selection on the frozen screen and adjust it. Press Space to switch to window mode. Add --copy, --save, --annotate or --pin to choose what happens next.' },
  { icon: 'app-window', title: 'Window', cmd: 'easyshot capture window', text: 'Hover to highlight a window, click to take it.' },
  { icon: 'monitor', title: 'Full screen', cmd: 'easyshot capture fullscreen', text: 'The output under the pointer.' },
  { icon: 'timer', title: 'Timed', cmd: 'easyshot capture window --timed 5', text: 'Pick first, then a countdown runs and a fresh frame is taken. Add --timed to region, window or fullscreen.' },
  { icon: 'unfold-vertical', title: 'Scrolling', cmd: 'easyshot capture scroll', text: 'Select an area, then scroll. easyshot joins it into one tall image. Press the shortcut again, or Stop, to finish.' },
  { icon: 'scan-text', title: 'Text', cmd: 'easyshot ocr', text: 'Reads the text, or a QR code, in an area and copies it.' },
]

const editorKeys = [
  ['Arrow keys', 'Nudge by 1 px (Shift: 10 px)'],
  ['M', 'Highlighter; press again to switch freehand and straight (Shift snaps to 15°)'],
  ['Ctrl D', 'Duplicate the selection'],
  ['Ctrl Shift C', 'Copy the image'],
  ['Space + drag', 'Pan the canvas'],
  ['Mouse wheel', 'Zoom about the pointer (two-finger touchpad scroll pans)'],
  ['Tab', 'Select the next shape'],
]

const files = [
  ['Settings', '~/.config/easyshot/config.toml'],
  ['Screenshots', '~/.local/share/easyshot/library/'],
  ['Recordings', '~/.local/share/easyshot/recordings/'],
  ['Logs and crash reports', '~/.local/state/easyshot/log/'],
  ['Default save folder', '~/Pictures/Screenshots'],
]

const trouble = [
  { q: 'A key bind does nothing', a: 'Run the command from a terminal, for example <code>easyshot capture region</code>. If it says the daemon is not running, start it with <code>systemctl --user start easyshot.service</code>. Then check that the bind line is in your compositor config and reload it.' },
  { q: 'OCR or recording items are greyed out', a: 'A tool is missing. Install Tesseract for text, or gpu-screen-recorder or wf-recorder for recording. <code>easyshot doctor</code> lists what is missing.' },
  { q: 'Window mode shows a list instead of a highlight', a: 'Your compositor does not report window positions. Pick the window from the list; the capture is the same.' },
  { q: 'The pointer disappears when I press Space in the editor', a: 'Your compositor hides the pointer when you press a key, until you move the mouse. On Hyprland turn off <code>cursor:hide_on_key_press</code>; on sway, <code>seat * hide_cursor when-typing disable</code>.' },
  { q: 'My settings file has an error', a: 'easyshot keeps running on the last good settings, never overwrites your file, and tells you the line and column. Fix it and save; changes apply live.' },
  { q: 'Something crashed', a: 'The next start offers “Open crash report”. Run <code>easyshot doctor --bundle</code> to make a report with your logs (no images), look it over, and attach it to an <a class="text-accent underline" href="https://github.com/mah3uz/easyshot/issues">issue</a>.' },
]

// Scroll spy for the table of contents.
const current = ref(steps[0]!.id)
let spy: IntersectionObserver | undefined
onMounted(() => {
  spy = new IntersectionObserver(
    (entries) => {
      const visible = entries.filter((e) => e.isIntersecting).sort((a, b) => a.boundingClientRect.top - b.boundingClientRect.top)
      if (visible[0]) current.value = visible[0].target.id
    },
    { rootMargin: '-20% 0px -65% 0px' },
  )
  for (const s of [...steps.map((s) => s.id), 'troubleshooting']) {
    const el = document.getElementById(s)
    if (el) spy.observe(el)
  }
})
onBeforeUnmount(() => spy?.disconnect())

useHead({
  script: [
    {
      type: 'application/ld+json',
      innerHTML: JSON.stringify({
        '@context': 'https://schema.org',
        '@type': 'HowTo',
        name: 'How to take screenshots and record your screen on Wayland with easyshot',
        description: 'Install easyshot, bind your keys, and take, annotate and record your first capture.',
        image: `${siteUrl}/og.png`,
        totalTime: 'PT10M',
        tool: [{ '@type': 'HowToTool', name: 'A Wayland compositor: Hyprland, sway or niri' }],
        supply: [{ '@type': 'HowToSupply', name: 'easyshot' }],
        step: steps.map((s, i) => ({
          '@type': 'HowToStep',
          position: i + 1,
          name: s.title,
          url: `${siteUrl}/how-to#${s.id}`,
        })),
      }),
    },
    {
      type: 'application/ld+json',
      innerHTML: JSON.stringify({
        '@context': 'https://schema.org',
        '@type': 'BreadcrumbList',
        itemListElement: [
          { '@type': 'ListItem', position: 1, name: 'Home', item: siteUrl },
          { '@type': 'ListItem', position: 2, name: 'How-to', item: `${siteUrl}/how-to` },
        ],
      }),
    },
  ],
})
</script>

<template>
  <div>
    <!-- Header -->
    <section class="relative overflow-hidden pt-[130px] pb-16 sm:pt-[150px]">
      <div class="pointer-events-none absolute top-[-240px] left-1/2 -z-10 h-[600px] w-[1000px] -translate-x-1/2 rounded-full bg-[radial-gradient(closest-side,var(--glow),transparent)]" aria-hidden="true" />
      <div class="container-x grid justify-items-center gap-6 text-center">
        <nav aria-label="Breadcrumb" class="rise text-[13px] text-fg-3">
          <NuxtLink to="/" class="hover:text-accent">Home</NuxtLink> <span class="mx-1.5">/</span> <span class="text-fg-2">How-to</span>
        </nav>
        <h1 class="rise max-w-[820px] text-[clamp(38px,6.4vw,68px)] leading-[1.04]" style="--d: 0.08s">
          From install to your <span class="grad-text">first screenshot</span>.
        </h1>
        <p class="rise max-w-[600px] text-[clamp(17px,2vw,20px)] text-fg-2" style="--d: 0.16s">
          Eight short steps. About ten minutes, most of it spent choosing which keys to use.
        </p>
        <div class="rise flex flex-wrap justify-center gap-2" style="--d: 0.24s">
          <span v-for="c in ['Hyprland', 'sway', 'niri']" :key="c" class="rounded-full border border-line bg-surface px-3 py-1 text-[13px] font-medium text-fg-2 shadow-soft">{{ c }}</span>
        </div>
      </div>
    </section>

    <div class="container-x grid gap-12 pb-28 lg:grid-cols-[220px_1fr] lg:gap-16">
      <!-- TOC -->
      <aside class="hidden lg:block">
        <nav class="sticky top-28 grid gap-1" aria-label="On this page">
          <p class="mb-2 text-xs font-semibold tracking-wider text-fg-3 uppercase">On this page</p>
          <a
            v-for="(s, i) in steps"
            :key="s.id"
            :href="`#${s.id}`"
            class="group flex items-center gap-3 rounded-lg px-2 py-1.5 text-[14px] transition-colors"
            :class="current === s.id ? 'bg-accent-soft font-medium text-accent' : 'text-fg-2 hover:text-fg'"
          >
            <span class="grid size-5 shrink-0 place-items-center rounded-full border text-[11px] transition-colors" :class="current === s.id ? 'border-accent bg-accent text-[#032830]' : 'border-line-strong'">{{ i + 1 }}</span>
            {{ s.title }}
          </a>
          <a href="#troubleshooting" class="mt-2 rounded-lg px-2 py-1.5 text-[14px] transition-colors" :class="current === 'troubleshooting' ? 'bg-accent-soft font-medium text-accent' : 'text-fg-2 hover:text-fg'">Troubleshooting</a>
        </nav>
      </aside>

      <article class="grid min-w-0 gap-20">
        <!-- 1 -->
        <HowStep :n="1" id="requirements" title="Check what you need">
          <div class="grid gap-4 sm:grid-cols-3">
            <div v-for="n in needs" :key="n.title" class="card grid content-start gap-3 p-5">
              <span class="icon-tile"><Icon :name="n.icon" :size="20" /></span>
              <h3 class="text-base">{{ n.title }}</h3>
              <p class="text-[14px] leading-relaxed text-fg-2">{{ n.text }}</p>
            </div>
          </div>
        </HowStep>

        <!-- 2 -->
        <HowStep :n="2" id="install" title="Install easyshot">
          <p>Pick your distribution. The package installs the app and the <code>easyshot</code> command, and enables the background service.</p>
          <InstallTabs />
        </HowStep>

        <!-- 3 -->
        <HowStep :n="3" id="start" title="Start the service">
          <p>
            easyshot runs as one background service, <code>easyshot.service</code>, with a tray icon. The package starts it for you. Built from source, start it yourself; your first
            command also starts it through D-Bus.
          </p>
          <CodeBlock prompt code="systemctl --user enable --now easyshot.service" />
          <p>Then ask easyshot what your desktop supports. It lists each capability, which provider it chose and why, and any tool that is missing.</p>
          <CodeBlock prompt code="easyshot doctor" />
          <Callout>To stop it starting for your user, run <code>systemctl --user mask --now easyshot.service</code>. A plain <code>disable</code> does not override the package.</Callout>
        </HowStep>

        <!-- 4 -->
        <HowStep :n="4" id="shortcuts" title="Bind your keys">
          <p>
            Wayland apps cannot grab global keys, so your compositor binds them, and easyshot sets none until you choose. On first run, <strong>Set your shortcuts</strong> suggests keys; later, use <strong>Settings › Shortcuts</strong>. easyshot writes your keys to its own file in <code>~/.config/easyshot/</code> and keeps it up to date. You add one line to your compositor config, once, and press <strong>Copy Line</strong> to get it. <strong>Check</strong> shows whether it is Included. easyshot never edits your config. The file it writes looks like the lines below.
          </p>
          <BindSnippet />
          <div class="overflow-hidden rounded-[18px] border border-line">
            <table class="w-full text-left text-[14.5px]">
              <caption class="sr-only">Global actions you can bind</caption>
              <thead class="bg-surface-2 text-xs tracking-wider text-fg-3 uppercase">
                <tr><th scope="col" class="px-4 py-3 font-semibold">Action</th><th scope="col" class="hidden px-4 py-3 font-semibold sm:table-cell">Command</th></tr>
              </thead>
              <tbody class="divide-y divide-line bg-surface">
                <tr v-for="k in keys" :key="k[1]" class="transition-colors hover:bg-surface-2">
                  <td class="px-4 py-3 text-fg-2">{{ k[0] }}</td>
                  <td class="hidden px-4 py-3 font-mono text-[13px] text-fg-3 sm:table-cell">easyshot {{ k[1] }}</td>
                </tr>
              </tbody>
            </table>
          </div>
          <Callout>Press <strong>Check</strong> in Settings › Shortcuts to see which keys are already bound in your compositor.</Callout>
        </HowStep>

        <!-- 5 -->
        <HowStep :n="5" id="capture" title="Take a screenshot">
          <p>Press the key you bound to <code>easyshot capture region</code>. The screen freezes, you drag an area, and the capture lands as a card in the preview deck, where you can copy, save, annotate, pin or drag it into another app or a file manager. Once auto-save has saved it, Save becomes Show in Folder.</p>
          <div class="grid gap-3 sm:grid-cols-2">
            <div v-for="c in captures" :key="c.title" class="card card-hover grid content-start gap-2.5 p-5">
              <h3 class="flex items-center gap-2 text-base"><Icon :name="c.icon" :size="18" class="text-accent" />{{ c.title }}</h3>
              <p class="text-[14px] leading-relaxed text-fg-2">{{ c.text }}</p>
              <code class="w-fit rounded-md bg-surface-2 px-2 py-1 text-[12.5px] text-fg-2">{{ c.cmd }}</code>
            </div>
          </div>
        </HowStep>

        <!-- 6 -->
        <HowStep :n="6" id="annotate" title="Annotate and share">
          <p>
            Choose <strong>Annotate</strong> on a deck card, or capture with <code>--annotate</code>. Add arrows, shapes, text and steps, with each tool's options (colour, stroke, arrowheads, font) in a panel under its button, blur what is private with <strong>Smart Redaction</strong> (it says what it hid and offers Undo), highlight text with the <strong>Highlighter</strong>, then set a background,
            padding, shadow and canvas shape in the inspector on the right. Save keeps your edits editable; Export writes PNG or JPEG.
          </p>
          <div class="grid gap-2 sm:grid-cols-2">
            <div v-for="k in editorKeys" :key="k[0]" class="flex items-center justify-between gap-4 rounded-xl border border-line bg-surface px-4 py-3 text-[14px]">
              <span class="text-fg-2">{{ k[1] }}</span>
              <span class="flex shrink-0 gap-1"><kbd v-for="part in k[0].split(' ')" :key="part">{{ part }}</kbd></span>
            </div>
          </div>
          <Callout>Smart Redaction only covers text Tesseract can read. Look over the result before you share.</Callout>
        </HowStep>

        <!-- 7 -->
        <HowStep :n="7" id="record" title="Record your screen">
          <p>Press the key you bound to <code>easyshot record</code>, pick an area, a display or a window, system sound is recorded unless you turn it off, and the microphone only if you choose one. After the countdown a small bar lets you stop, pause, restart or discard.</p>
          <CodeBlock prompt code="easyshot record          # start
easyshot record stop     # stop and open the trim viewer" />
          <p>The recording appears as a deck card and opens in the trim viewer, where you cut it and export it. Saved recordings and exports go to <code>~/Videos/easyshot/</code>. One recording runs at a time.</p>
        </HowStep>

        <!-- 8 -->
        <HowStep :n="8" id="settings" title="Settings and files">
          <p>Easyshot in your app menu, or <code>easyshot</code> on its own, opens the Home window: your media gallery and Settings, one tab each. The tray's Settings item and <code>easyshot settings</code> open it on Settings. Every change applies at once. You can also edit <code>config.toml</code> by hand: easyshot watches it and applies changes live.</p>
          <dl class="divide-y divide-line overflow-hidden rounded-[18px] border border-line bg-surface">
            <div v-for="f in files" :key="f[0]" class="grid gap-1 px-4 py-3 sm:grid-cols-[200px_1fr]">
              <dt class="text-[14px] font-medium">{{ f[0] }}</dt>
              <dd class="m-0 font-mono text-[13px] break-all text-fg-2">{{ f[1] }}</dd>
            </div>
          </dl>
        </HowStep>

        <!-- Troubleshooting -->
        <section id="troubleshooting" class="grid scroll-mt-28 gap-6">
          <div class="grid gap-3">
            <span class="eyebrow"><Icon name="circle-alert" :size="15" />Troubleshooting</span>
            <h2 class="text-[clamp(28px,3.6vw,40px)]">If something is not right</h2>
            <p class="text-fg-2">Start with <code>easyshot doctor</code>. Set <code>EASYSHOT_LOG</code> to change what is logged.</p>
          </div>
          <FaqList :items="trouble" />
          <p class="text-[14.5px] text-fg-2">
            Still stuck? Read the full <a :href="`${repo}/blob/main/docs/MANUAL.md`" rel="noopener" class="text-accent underline">manual</a> or
            <a :href="`${repo}/issues`" rel="noopener" class="text-accent underline">open an issue</a>.
          </p>
        </section>
      </article>
    </div>
  </div>
</template>
