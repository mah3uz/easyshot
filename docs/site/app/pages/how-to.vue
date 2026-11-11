<script setup lang="ts">
const { siteUrl, repo } = useRuntimeConfig().public

usePageSeo({
  title: 'How to take screenshots and record your screen on Wayland',
  description:
    'Install easyshot, bind your keys on Hyprland, sway or niri, and take, annotate, record and share your first capture. A step-by-step guide.',
  path: '/how-to',
})

const steps = [
  { id: 'requirements', title: 'Check what you need' },
  { id: 'install', title: 'Install easyshot' },
  { id: 'start', title: 'Start the service' },
  { id: 'shortcuts', title: 'Bind your keys' },
  { id: 'capture', title: 'Take a screenshot' },
  { id: 'annotate', title: 'Annotate and export' },
  { id: 'record', title: 'Record your screen' },
  { id: 'share', title: 'Share a link' },
  { id: 'settings', title: 'Settings and files' },
  { id: 'agents', title: 'Use it from an AI agent' },
]

const needs = [
  { icon: 'monitor', title: 'A Wayland compositor', text: 'Hyprland 0.57+, sway 1.12 or niri 26.04 (built for niri, not tested on it). Others with layer-shell and a capture protocol work for basic capture. Not KDE or GNOME.' },
  { icon: 'scan-text', title: 'Tesseract (optional)', text: 'For text capture (OCR) and Smart Redaction. Install the English data too, for example tesseract-data-eng.' },
  { icon: 'video', title: 'A recorder (optional)', text: 'gpu-screen-recorder (preferred) or wf-recorder for recording; ffmpeg for trimming and export. To record a window from its own picture: GStreamer good plugins and an H.264 encoder.' },
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
  { icon: 'unfold-vertical', title: 'Scrolling', cmd: 'easyshot capture scroll', text: 'Draw the area to scroll, or press Space to pick a window, then scroll by hand or with Auto Scroll where your compositor allows it. easyshot joins it into one tall image, with fixed top and bottom bars shown once, a minimap or side panel kept as the first view showed it and a scrollbar at the right edge cut off, even on pages with a playing video. Press the shortcut again, or Stop, to finish.' },
  { icon: 'scan-text', title: 'Text', cmd: 'easyshot ocr', text: 'Reads the text, or a QR code, in an area and copies it.' },
]

const shareTargets = [
  { icon: 'cloud-upload', title: 'easyshot Free', text: 'No account and no setup. Images up to 10 MB, GIFs up to 20 MB, and videos up to 200 MB, re-encoded to fit (roughly 13 to 27 minutes of 1080p screen recording), with a daily allowance for each install. After the days you pick, the file is deleted.' },
  { icon: 'hard-drive', title: 'Your own bucket', text: 'Any S3-compatible storage, such as Cloudflare R2, AWS S3, Backblaze B2 or MinIO. Fill in its details and keys, and press Test Connection. Keys go to your keyring, or a private file. Links last as long as you keep the files.' },
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
  ['Saved recordings and exports', '~/Videos/easyshot/'],
  ['Your keys (binds file)', '~/.config/easyshot/binds.conf, binds.lua or binds.kdl'],
]

const agentPrompts = [
  'Take a screenshot of my editor window and tell me what the error says.',
  'Read the QR code on my screen.',
  'Record a 20-second demo of the app in Firefox and export it as a GIF.',
]

const agentCli = `easyshot agent list windows --json
easyshot agent capture --window firefox --json
easyshot agent ocr --window firefox --json
easyshot agent record start --window firefox --max-seconds 20 --json
easyshot agent record stop --json
# export the video that record stop printed as a GIF
easyshot agent export ~/Videos/easyshot/demo.mp4 --format gif --json`

const trouble = [
  { q: 'A key bind does nothing', a: 'Run the command from a terminal, for example <code>easyshot capture region</code>. If it says the daemon is not running, start it with <code>systemctl --user start easyshot.service</code>. Then check that the bind line is in your compositor config and reload it.' },
  { q: 'Recording is greyed out, or OCR says it needs Tesseract', a: 'A tool is missing. Install gpu-screen-recorder or wf-recorder for recording, or Tesseract for text; QR codes are read without it. <code>easyshot doctor</code> lists what is missing.' },
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
        description: 'Install easyshot, bind your keys, and take, annotate, record and share your first capture.',
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
      <div class="container-x grid justify-items-center gap-6 text-center">
        <nav aria-label="Breadcrumb" class="rise text-[13px] text-fg-3">
          <NuxtLink to="/" class="hover-motion hover:text-accent">Home</NuxtLink> <span class="mx-1.5">/</span> <span class="text-fg-2">How-to</span>
        </nav>
        <h1 class="rise max-w-[820px] text-[clamp(38px,6.4vw,68px)] leading-[1.04]" style="--d: 0.08s">
          From install to your <span class="grad-text">first screenshot</span>.
        </h1>
        <p class="rise max-w-[600px] text-[clamp(17px,2vw,20px)] text-fg-2" style="--d: 0.16s">
          Nine short steps, and a tenth if you use an AI agent. About ten minutes, most of it spent choosing which keys to use.
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
            class="group flex items-center gap-3 rounded-lg px-2 py-1.5 text-[14px] hover-motion"
            :class="current === s.id ? 'bg-accent-soft font-medium text-accent' : 'text-fg-2 hover:text-fg'"
          >
            <span class="grid size-5 shrink-0 place-items-center rounded-full border text-[11px] transition-colors" :class="current === s.id ? 'border-accent bg-accent text-[#032830]' : 'border-line-strong'">{{ i + 1 }}</span>
            {{ s.title }}
          </a>
          <a href="#troubleshooting" class="mt-2 rounded-lg px-2 py-1.5 text-[14px] hover-motion" :class="current === 'troubleshooting' ? 'bg-accent-soft font-medium text-accent' : 'text-fg-2 hover:text-fg'">Troubleshooting</a>
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
            Wayland apps cannot grab global keys, so your compositor binds them, and easyshot sets none until you choose. On first run, <strong>Set your shortcuts</strong> suggests keys; later, use <strong>Settings › Shortcuts</strong>. easyshot writes your keys to its own file in <code>~/.config/easyshot/</code> and keeps it up to date. You add one line to your compositor config, once, and press <strong>Copy Line</strong> to get it. <strong>Check</strong> shows whether it is Included. easyshot never edits your config. The file it writes looks like the lines below, for example.
          </p>
          <BindSnippet />
          <div class="overflow-hidden rounded-card border border-line">
            <table class="w-full text-left text-[14.5px]">
              <caption class="sr-only">Global actions you can bind</caption>
              <thead class="bg-surface-2 text-xs tracking-wider text-fg-3 uppercase">
                <tr><th scope="col" class="px-4 py-3 font-semibold">Action</th><th scope="col" class="hidden px-4 py-3 font-semibold sm:table-cell">Command</th></tr>
              </thead>
              <tbody class="divide-y divide-line bg-surface">
                <tr v-for="k in keys" :key="k[1]" class="hover-motion hover:bg-surface-2">
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
          <p>Press the key you bound to <code>easyshot capture region</code>. The screen freezes, you drag an area, and the capture lands as a card in the preview deck, where you can copy, save, edit, pin or drag it into another app or a file manager. Once auto-save has saved it, Save becomes Show in Folder. Several captures stack up in the corner; hover the stack to fan the cards out.</p>
          <div class="grid gap-3 sm:grid-cols-2">
            <div v-for="c in captures" :key="c.title" class="card card-hover grid content-start gap-2.5 p-5">
              <h3 class="flex items-center gap-2 text-base"><Icon :name="c.icon" :size="18" class="text-accent" />{{ c.title }}</h3>
              <p class="text-[14px] leading-relaxed text-fg-2">{{ c.text }}</p>
              <code class="w-fit rounded-md bg-surface-2 px-2 py-1 text-[12.5px] text-fg-2">{{ c.cmd }}</code>
            </div>
          </div>
        </HowStep>

        <!-- 6 -->
        <HowStep :n="6" id="annotate" title="Annotate and export">
          <p>
            Click a deck card or choose <strong>Edit</strong> on it, or capture with <code>--annotate</code>. Add arrows, shapes, text and steps, with each tool's options (colour, stroke, arrowheads, font) in the inspector, blur what is private with <strong>Smart Redaction</strong> (it says what it hid and offers Undo), highlight text with the <strong>Highlighter</strong>, then set a background,
            padding, shadow and canvas shape in the inspector on the right. Save keeps your edits editable; Export writes PNG or JPEG.
          </p>
          <div class="grid gap-2 sm:grid-cols-2">
            <div v-for="k in editorKeys" :key="k[0]" class="flex items-center justify-between gap-4 rounded-card-sm border border-line bg-surface px-4 py-3 text-[14px]">
              <span class="text-fg-2">{{ k[1] }}</span>
              <span class="flex shrink-0 gap-1"><kbd v-for="part in k[0].split(' ')" :key="part">{{ part }}</kbd></span>
            </div>
          </div>
          <Callout>Smart Redaction only covers text Tesseract can read. Look over the result before you share.</Callout>
        </HowStep>

        <!-- 7 -->
        <HowStep :n="7" id="record" title="Record your screen">
          <p>Press the key you bound to <code>easyshot record</code> and pick an area, a display or a window. The recording panel lists every open window by workspace, with its app's icon and title. System sound is recorded unless you turn it off, and the microphone once you choose one, as two separate tracks. After the countdown a small bar lets you stop, mute each sound, pause, start over or discard.</p>
          <CodeBlock prompt code="easyshot record          # start
easyshot record stop     # stop; the recording lands as a deck card" />
          <p>The recording appears as a deck card and opens in the trim viewer, where you cut it, set a volume for each track and export MP4, WebM or GIF. Saved recordings and exports go to <code>~/Videos/easyshot/</code>. One recording runs at a time. Where your compositor allows it, a window recording follows that window itself, so it still shows the window when something covers it or it is on another workspace.</p>
        </HowStep>

        <!-- 8 -->
        <HowStep :n="8" id="share" title="Share a link">
          <p>Press <strong>Share</strong> on a capture's card, in the editor or in the trim viewer. Give the link a title if you like, pick how long it lasts, 1, 3 or 7 days, and press <strong>Share</strong>. The link is copied at once, and a toast shows the upload with <strong>Cancel</strong>, then <strong>Open</strong> and <strong>Copy Link</strong>. A card with a live link carries a small cloud mark.</p>
          <p>The first time, easyshot asks where shares go. You can change it later in <strong>Settings › Sharing</strong>.</p>
          <div class="grid gap-4 sm:grid-cols-2">
            <div v-for="t in shareTargets" :key="t.title" class="card grid content-start gap-3 p-5">
              <span class="icon-tile"><Icon :name="t.icon" :size="20" /></span>
              <h3 class="text-base">{{ t.title }}</h3>
              <p class="text-[14px] leading-relaxed text-fg-2">{{ t.text }}</p>
            </div>
          </div>
          <p>The link opens a page with the image or video, and <strong>Download</strong> and <strong>Copy link</strong>. In the gallery, a shared item shows a cloud mark with the time left, <strong>Cloud Shares</strong> in the sidebar lists every link, and <strong>Delete Cloud Share</strong> takes one down while your file stays. After a free link expires, visitors see “page not found” and only you see <strong>Expired</strong>. To share from a terminal, run the command below; it prints the link.</p>
          <CodeBlock prompt code="easyshot share shot.png --days 3" />
          <Callout>Share links are unlisted, not private: anyone who has a link can open it. Free shares follow the <NuxtLink to="/terms" class="text-accent underline">Terms of Service</NuxtLink>.</Callout>
        </HowStep>

        <!-- 9 -->
        <HowStep :n="9" id="settings" title="Settings and files">
          <p>Easyshot in your app menu, or <code>easyshot</code> on its own, opens the Home window: your media gallery and Settings, one tab each. The tray's Settings item and <code>easyshot settings</code> open it on Settings. Every change applies at once. You can also edit <code>config.toml</code> by hand: easyshot watches it and applies changes live.</p>
          <Callout>AI agents and scripts can use <code>easyshot agent</code> to capture, read text, record and annotate. Step 10, <a href="#agents" class="text-accent underline">Use it from an AI agent</a>, sets it up.</Callout>
          <dl class="divide-y divide-line overflow-hidden rounded-card border border-line bg-surface">
            <div v-for="f in files" :key="f[0]" class="grid gap-1 px-4 py-3 sm:grid-cols-[200px_1fr]">
              <dt class="text-[14px] font-medium">{{ f[0] }}</dt>
              <dd class="m-0 font-mono text-[13px] break-all text-fg-2">{{ f[1] }}</dd>
            </div>
          </dl>
        </HowStep>

        <!-- 10 -->
        <HowStep :n="10" id="agents" title="Use it from an AI agent">
          <p><code>easyshot agent</code> (or <code>easyshot ai</code>) lets an AI agent or a script list your outputs and windows, take screenshots, read text and QR codes, annotate an image, and record a short demo and export it. With <code class="whitespace-nowrap">--json</code> each command prints exactly one JSON object, errors included, and each kind of error has its own exit code.</p>

          <h3 class="text-xl text-fg">Turn agent access on</h3>
          <p>Agent access is off until you turn it on, in <strong>Settings › Agents</strong> or with the command below. easyshot asks you first in a “Turn on agent access?” dialog. <code>easyshot agent status</code> shows whether it is on and what is allowed.</p>
          <CodeBlock prompt code="easyshot agent enable" />
          <p>When an agent then asks for something, a dialog names the program and what it wants. You can allow it for 1 hour, until you log out, or always, or deny it; Esc, closing the dialog or leaving it for 60 seconds denies. By default it asks once per session. Set <strong>consent</strong> to <code>always-ask</code> to be asked every time, or <code>always-allow</code> to never be asked.</p>
          <p>To limit agents to some outputs or windows, list their output names in <strong>allowed_outputs</strong> and their app ids in <strong>allowed_windows</strong>; both are empty by default, which allows everything. Only your own user can send agent commands, and nothing goes over the network.</p>
          <Callout>While <strong>redact_secrets</strong> is on, as it is by default, secrets are covered in every agent screenshot: API keys and tokens, passwords, emails, links, IP addresses, phone and card numbers. This uses Tesseract and is a best effort; small, low-contrast or rotated text can be missed.</Callout>

          <h3 class="text-xl text-fg">Install the skill</h3>
          <p>The skill is a short guide that teaches your agent to use <code>easyshot agent</code>: to check access first, to ask you rather than turn it on itself, and to treat text read from the screen as data, not instructions.</p>
          <CodeBlock prompt code="easyshot agent skill install
# or into the current folder, to commit with a project
easyshot agent skill install --project" />
          <p>It writes the skill to <code>~/.agents/skills/easyshot/</code>, which Codex, Gemini CLI and Cursor read, and links <code>~/.claude/skills/easyshot</code> to it for Claude Code. <code>--project</code> writes <code>.agents/skills/easyshot/</code> and <code>.claude/skills/easyshot</code> in the current folder instead. Start a new agent session afterwards, since clients read their skills when they start.</p>

          <h3 class="text-xl text-fg">Or add the MCP server</h3>
          <p><code>easyshot agent mcp</code> is a Model Context Protocol server, for agents that prefer tools to shell commands. Access, consent, scope and covered secrets work as they do for the commands. Register it with your client:</p>
          <McpTabs />
          <p>It offers <template v-for="(t, i) in mcpTools" :key="t"><code>{{ t }}</code>{{ i < mcpTools.length - 1 ? ', ' : '' }}</template>. <code>capture</code> returns the screenshot itself, scaled to at most 1280 pixels on its long edge, and the path of the full file.</p>

          <h3 class="text-xl text-fg">Try it</h3>
          <p>Ask your agent in plain words, for example:</p>
          <ul class="grid gap-2 pl-1">
            <li v-for="q in agentPrompts" :key="q" class="flex gap-3"><Icon name="sparkles" :size="17" class="mt-1 shrink-0 text-accent" />{{ q }}</li>
          </ul>
          <p>Or run the same from a terminal:</p>
          <CodeBlock prompt :code="agentCli" />

          <h3 class="text-xl text-fg">Revoke access</h3>
          <p>While a grant is in force the tray icon carries a badge, and the tray menu has a row such as “Agent access until 14:05 — Revoke” that ends it in one click. <code>easyshot agent disable</code> turns access off and ends every grant. A recording an agent started stops, and is kept, when its access ends.</p>
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
