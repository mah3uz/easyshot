<script setup lang="ts">
const { repo, siteUrl } = useRuntimeConfig().public

usePageSeo({
  title: 'easyshot — screenshots, annotation and screen recording for Linux Wayland',
  description:
    'A free, open-source alternative to CleanShot for Linux. Capture an area, window, text or scrolling page, annotate it in a full editor, and record your screen on Hyprland, sway and niri.',
  path: '/',
})

const faq = [
  {
    q: 'Is easyshot free?',
    a: 'Yes. It is free and open source. There is no account, no subscription and no paid tier.',
  },
  {
    q: 'Which desktops does it work on?',
    a: 'Version 1 targets Hyprland 0.57+, sway 1.12 and niri 26.04; niri support is built but not tested on niri. Other Wayland compositors with layer-shell and a capture protocol should work for basic capture. KDE and GNOME are not supported.',
  },
  {
    q: 'How do I install it?',
    a: 'There are two AUR packages, <code>easyshot</code> and <code>easyshot-bin</code>, which are not published yet, and no packages for other distributions. Build it with <code>cargo build --release -p easyshot</code>, or on Arch run <code>just install</code> from a checkout. See <a class="text-accent underline" href="/#install">Install</a>.',
  },
  {
    q: 'How do I set the keyboard shortcuts?',
    a: 'Wayland apps cannot grab global keys, and easyshot sets none until you choose them. Pick your keys in Settings › Shortcuts or in the first-run step, which suggests the Print keys where they are free. easyshot writes them to its own binds file for Hyprland, sway or niri, and you add the one include line it shows you to your config, once; Check tells you whether it is included. easyshot never edits your config itself.',
  },
  {
    q: 'Does it upload my screenshots?',
    a: 'No. Everything stays on your machine: captures, recordings, logs and crash reports. Cloud sharing is coming soon; for now the Cloud Share action on a card only says so.',
  },
  {
    q: 'What does recording need?',
    a: "gpu-screen-recorder (preferred) or wf-recorder, and ffmpeg to trim and export. Without a recorder, the tray's Record item is greyed out and tells you what to install. Recording a window from its own picture needs GStreamer's good plugins and an H.264 encoder; without them a window recording records the window's area on screen. Text capture needs Tesseract; QR codes are read without it.",
  },
  {
    q: 'Can I edit recordings into a video?',
    a: 'The trim viewer cuts a recording and exports MP4, WebM or GIF, with a size, a quality, a colour look and a volume for each sound track. A Video Studio with a timeline, zooms and captions is coming soon.',
  },
  {
    q: 'Can AI agents use it?',
    a: 'Yes, once you allow it. <code>easyshot agent</code> lets an agent or script list windows, take screenshots with secrets covered, read text, record and annotate, and <code>easyshot agent mcp</code> is a Model Context Protocol server. Agent access is off until you turn it on, asks you in a dialog first, and a tray badge with one-click Revoke shows while it is granted.',
  },
]

const why = [
  { icon: 'heart', title: 'Free and open source', text: 'Read the code, change it, ship it.' },
  { icon: 'shield-check', title: 'Private by default', text: 'No account and no uploads. Crash reports stay on your disk.' },
  { icon: 'zap', title: 'Native and fast', text: 'Written in Rust with GPUI, rendered on your GPU.' },
  { icon: 'keyboard', title: 'Keyboard first', text: 'Every action has a command, and your compositor binds the keys.' },
]

const editorPoints = [
  'Arrows, shapes, pen, highlighter, text, numbered steps, blur and pixelate',
  'Crop with aspect presets, rotate and flip, and undo for all of it',
  'Background, padding, corners, shadow, border, watermark and presets',
  'Camera 3D and progressive blur for a polished look',
  'Unsaved edits come back after a crash',
]

useHead({
  script: [
    {
      type: 'application/ld+json',
      innerHTML: JSON.stringify([
        {
          '@context': 'https://schema.org',
          '@type': 'SoftwareApplication',
          name: 'easyshot',
          applicationCategory: 'MultimediaApplication',
          applicationSubCategory: 'Screenshot tool',
          operatingSystem: 'Linux (Wayland)',
          description:
            'Screenshots, annotation and screen recording for Linux on Wayland. A free, open-source alternative to CleanShot.',
          url: siteUrl,
          image: `${siteUrl}/og.png`,
          codeRepository: repo,
          offers: { '@type': 'Offer', price: '0', priceCurrency: 'USD' },
        },
        {
          '@context': 'https://schema.org',
          '@type': 'FAQPage',
          mainEntity: faq.map((f) => ({
            '@type': 'Question',
            name: f.q,
            acceptedAnswer: { '@type': 'Answer', text: f.a.replace(/<[^>]+>/g, '') },
          })),
        },
      ]),
    },
  ],
})
</script>

<template>
  <div>
    <!-- Hero -->
    <section class="relative overflow-hidden pt-[130px] pb-20 sm:pt-[150px]">

      <div class="container-x grid justify-items-center gap-7 text-center">
        <img src="/logo.svg" alt="easyshot logo" width="96" height="96" class="rise size-20 drop-shadow-[0_20px_30px_rgba(19,136,166,.35)] sm:size-24" />
        <a :href="repo" rel="noopener" class="rise group inline-flex items-center gap-2 rounded-full border border-line bg-surface py-1.5 pr-3.5 pl-1.5 text-[13.5px] text-fg-2 shadow-soft transition hover:border-accent" style="--d: 0.08s">
          <span class="rounded-full bg-accent-soft px-2 py-0.5 text-xs font-semibold text-accent">Open source</span>
          Free, built with Rust
          <Icon name="arrow-up-right" :size="14" class="transition-transform group-hover:translate-x-0.5 group-hover:-translate-y-0.5" />
        </a>
        <h1 class="rise max-w-[900px] text-[clamp(42px,7.4vw,84px)] leading-[1.02]" style="--d: 0.16s">
          Screenshots, done right<br class="hidden sm:block" />
          on <span class="grad-text">Wayland</span>.
        </h1>
        <p class="rise max-w-[640px] text-[clamp(17px,2.2vw,21px)] leading-relaxed text-fg-2" style="--d: 0.24s">
          easyshot is a free and open-source alternative to CleanShot for Linux. Capture, annotate and record your
          screen on Hyprland, sway and niri, from a key bind.
        </p>
        <div class="rise flex flex-wrap justify-center gap-3" style="--d: 0.32s">
          <NuxtLink to="/how-to" class="btn btn-primary">
            <Icon name="download" :size="18" /> Get started
          </NuxtLink>
          <a :href="repo" rel="noopener" class="btn btn-ghost">
            <svg width="18" height="18" viewBox="0 0 24 24" fill="currentColor" aria-hidden="true"><path d="M12 .5a11.5 11.5 0 0 0-3.64 22.41c.58.1.79-.25.79-.56v-2c-3.2.7-3.88-1.37-3.88-1.37-.53-1.33-1.28-1.69-1.28-1.69-1.05-.72.08-.7.08-.7 1.16.08 1.77 1.19 1.77 1.19 1.03 1.77 2.7 1.26 3.36.96.1-.75.4-1.26.73-1.55-2.55-.29-5.24-1.28-5.24-5.69 0-1.26.45-2.29 1.19-3.1-.12-.29-.52-1.46.11-3.05 0 0 .97-.31 3.17 1.18a11 11 0 0 1 5.77 0c2.2-1.49 3.17-1.18 3.17-1.18.63 1.59.23 2.76.11 3.05.74.81 1.19 1.84 1.19 3.1 0 4.42-2.7 5.39-5.26 5.68.41.36.78 1.06.78 2.14v3.17c0 .31.21.67.8.56A11.5 11.5 0 0 0 12 .5Z" /></svg>
            View on GitHub
          </a>
        </div>
        <p class="rise text-[13px] text-fg-3" style="--d: 0.4s">For Linux on Wayland · Build it from source today</p>
      </div>

      <div class="container-x mt-16 sm:mt-20">
        <HeroShot />
      </div>
    </section>

    <!-- Compositors -->
    <section class="border-y border-line bg-bg-2/50 py-10" aria-label="Supported compositors">
      <div class="container-x flex flex-wrap items-center justify-center gap-x-12 gap-y-4 text-fg-3">
        <span class="text-[13px] font-medium">Made for tiling Wayland</span>
        <span v-for="c in ['Hyprland', 'sway', 'niri']" :key="c" class="font-display text-2xl font-bold tracking-tight text-fg-2">{{ c }}</span>
        <span class="text-[13px]">+ basic capture on other Wayland compositors</span>
      </div>
    </section>

    <!-- All in one -->
    <section id="features" class="py-24 sm:py-32">
      <div class="container-x">
        <div v-reveal class="mx-auto mb-14 grid max-w-[720px] justify-items-center gap-4 text-center">
          <span class="eyebrow"><Icon name="sparkles" :size="15" />All in one</span>
          <h2 class="text-[clamp(34px,5vw,56px)] leading-[1.05]">One app for every kind of capture.</h2>
          <p class="text-[clamp(17px,2vw,20px)] text-fg-2">
            Screenshots, text, colours, scrolling pages and recordings. Everything you used three tools for, behind one set
            of keys.
          </p>
        </div>
        <FeatureBento />
      </div>
    </section>

    <!-- Annotate -->
    <section id="annotate" class="relative overflow-hidden bg-bg-2/60 py-24 sm:py-32">
      <div class="container-x grid grid-cols-[minmax(0,1fr)] gap-14">
        <div class="grid items-end gap-8 md:grid-cols-2">
          <div v-reveal class="grid gap-4">
            <span class="eyebrow"><Icon name="pencil-line" :size="15" />Annotate</span>
            <h2 class="text-[clamp(34px,5vw,56px)] leading-[1.05]">Mark it up.<br />Make it beautiful.</h2>
          </div>
          <p v-reveal="0.1" class="text-lg text-fg-2">
            A full editor with 13 tools opens right after the capture. Point things out, hide what is private, then put it
            on a background that is ready to share. Try the controls below.
          </p>
        </div>
        <div v-reveal="0.1">
          <EditorMock />
        </div>
        <ul class="grid gap-x-8 gap-y-3 sm:grid-cols-2 lg:grid-cols-3">
          <li v-for="(p, i) in editorPoints" :key="p" v-reveal="i * 0.05" class="flex gap-3 text-[15px] text-fg-2">
            <span class="mt-0.5 grid size-5 shrink-0 place-items-center rounded-full bg-accent-soft text-accent"><Icon name="check" :size="12" /></span>
            {{ p }}
          </li>
        </ul>
      </div>
    </section>

    <!-- Smart redaction -->
    <section class="py-24 sm:py-32">
      <div class="container-x grid items-center gap-12 md:grid-cols-2">
        <div v-reveal class="relative order-2 md:order-1">
          <div class="card relative overflow-hidden p-6">
            <div class="grid gap-3 font-mono text-[13px] sm:text-sm">
              <div v-for="row in [['email', 'jane@example.com'], ['token', 'sk_live_51HxQ…9fA2'], ['card', '4242 1234 5678 9010'], ['ip', '192.168.1.24']]" :key="row[0]" class="flex items-center gap-3">
                <span class="w-14 text-fg-3">{{ row[0] }}</span>
                <span class="rd-cover relative rounded-md px-2 py-1">{{ row[1] }}</span>
              </div>
            </div>
            <p class="mt-6 flex items-center gap-2 text-[13px] text-fg-3"><Icon name="eye-off" :size="15" class="text-accent" /> Added 4 redactions</p>
          </div>
        </div>
        <div v-reveal="0.1" class="order-1 grid gap-4 md:order-2">
          <span class="eyebrow"><Icon name="eye-off" :size="15" />Smart Redaction</span>
          <h2 class="text-[clamp(32px,4.4vw,50px)] leading-[1.05]">Hide secrets in one click.</h2>
          <p class="text-lg text-fg-2">
            easyshot reads the screenshot and covers what looks sensitive: e-mail addresses, links, IP addresses, phone and
            card numbers, API keys and tokens, and values after labels like <code>password:</code>. The covers are
            ordinary shapes, so you can move or remove them.
          </p>
          <p class="text-sm text-fg-3">Uses Tesseract. Check the result before you share: small or stylised text can be missed.</p>
        </div>
      </div>
    </section>

    <!-- Record -->
    <section id="record" class="bg-bg-2/60 py-24 sm:py-32">
      <div class="container-x grid items-center gap-12 md:grid-cols-[1fr_1.2fr]">
        <div v-reveal class="grid gap-4">
          <span class="eyebrow"><Icon name="video" :size="15" />Record</span>
          <h2 class="text-[clamp(32px,4.4vw,50px)] leading-[1.05]">Record your screen, with sound.</h2>
          <p class="text-lg text-fg-2">
            Record an area, a display or a window, with system sound and your microphone as two separate tracks. Mute
            either sound, pause, start over or discard from a small control bar, then trim the clip in the viewer.
          </p>
          <ul class="grid gap-2.5 pt-2 text-[15px] text-fg-2">
            <li class="flex items-center gap-3"><Icon name="zap" :size="17" class="text-accent" /> Uses gpu-screen-recorder, or wf-recorder</li>
            <li class="flex items-center gap-3"><Icon name="app-window" :size="17" class="text-accent" /> A window keeps recording when it is covered or on another workspace</li>
            <li class="flex items-center gap-3"><Icon name="timer" :size="17" class="text-accent" /> Countdown before it starts</li>
            <li class="flex items-center gap-3"><Icon name="film" :size="17" class="text-accent" /> Trim viewer with MP4, WebM and GIF export</li>
          </ul>
        </div>
        <div v-reveal="0.1"><RecordMock /></div>
      </div>
    </section>

    <!-- Keyboard / CLI -->
    <section class="py-24 sm:py-32">
      <div class="container-x grid items-center gap-12 md:grid-cols-[0.8fr_1.2fr]">
        <div v-reveal class="grid gap-4">
          <span class="eyebrow"><Icon name="keyboard" :size="15" />Keyboard first</span>
          <h2 class="text-[clamp(32px,4.4vw,50px)] leading-[1.05]">Your keys. Your compositor.</h2>
          <p class="text-lg text-fg-2">
            Every action is a command, so any key bind, launcher or script can drive easyshot. Settings › Shortcuts writes your
            keys to its own file, gives you the one line to include, and checks which keys are already taken. You might bind:
          </p>
          <div class="flex flex-wrap items-center gap-x-6 gap-y-3 pt-2 text-sm text-fg-2">
            <span class="flex items-center gap-1.5 whitespace-nowrap"><kbd>Super</kbd><kbd>Shift</kbd><kbd>4</kbd><span class="ml-1">capture an area</span></span>
            <span class="flex items-center gap-1.5 whitespace-nowrap"><kbd>Super</kbd><kbd>Shift</kbd><kbd>5</kbd><span class="ml-1">record</span></span>
          </div>
        </div>
        <div v-reveal="0.1"><BindSnippet /></div>
      </div>
    </section>

    <!-- Why -->
    <section class="bg-bg-2/60 py-24 sm:py-32">
      <div class="container-x">
        <div v-reveal class="mx-auto mb-14 grid max-w-[680px] justify-items-center gap-4 text-center">
          <span class="eyebrow">Why easyshot</span>
          <h2 class="text-[clamp(34px,5vw,56px)] leading-[1.05]">Built for Linux, not ported to it.</h2>
        </div>
        <div class="grid gap-4 sm:grid-cols-2 lg:grid-cols-4">
          <div v-for="(w, i) in why" :key="w.title" v-reveal="i * 0.07" class="card card-hover grid content-start gap-4 p-6">
            <span class="icon-tile"><Icon :name="w.icon" :size="21" /></span>
            <h3 class="text-lg">{{ w.title }}</h3>
            <p class="text-[14.5px] leading-relaxed text-fg-2">{{ w.text }}</p>
          </div>
        </div>
      </div>
    </section>

    <!-- Install -->
    <section id="install" class="py-24 sm:py-32">
      <div class="container-x grid items-start gap-12 md:grid-cols-[0.9fr_1.1fr]">
        <div v-reveal class="grid gap-4">
          <span class="eyebrow"><Icon name="download" :size="15" />Install</span>
          <h2 class="text-[clamp(32px,4.4vw,50px)] leading-[1.05]">Get it your way.</h2>
          <p class="text-lg text-fg-2">
            Build easyshot from source today. Two Arch (AUR) packages are on the way but not published yet; the package starts
            the background service for you. Then choose your keys and press one.
          </p>
          <NuxtLink to="/how-to" class="group mt-2 inline-flex w-fit items-center gap-1.5 font-semibold text-accent">
            Full setup guide <Icon name="arrow-up-right" :size="16" class="transition-transform group-hover:translate-x-0.5 group-hover:-translate-y-0.5" />
          </NuxtLink>
        </div>
        <div v-reveal="0.1"><InstallTabs /></div>
      </div>
    </section>

    <!-- FAQ -->
    <section id="faq" class="mb-24 bg-bg-2/60 py-24 sm:mb-28 sm:py-32">
      <div class="container-x max-w-[820px]">
        <div v-reveal class="mb-12 grid justify-items-center gap-4 text-center">
          <span class="eyebrow">FAQ</span>
          <h2 class="text-[clamp(34px,5vw,52px)]">Questions, answered.</h2>
        </div>
        <div v-reveal="0.05"><FaqList :items="faq" /></div>
      </div>
    </section>

    <!-- CTA -->
    <section class="pb-28">
      <div class="container-x">
        <div v-reveal class="relative overflow-hidden rounded-card bg-[#043040] px-6 py-16 text-center text-white shadow-float sm:px-12 sm:py-20">
          <div class="pointer-events-none absolute inset-0 bg-[radial-gradient(ellipse_at_top,rgba(98,237,212,.35),transparent_60%)]" aria-hidden="true" />
          <div class="relative grid justify-items-center gap-6">
            <img src="/logo.svg" alt="" width="72" height="72" class="animate-float" loading="lazy" />
            <h2 class="max-w-[640px] text-[clamp(32px,4.6vw,52px)] leading-[1.05]">Take better screenshots on Linux.</h2>
            <p class="max-w-[520px] text-lg text-[#a3c3c8]">Free, open source and private. Build it in a few minutes.</p>
            <div class="flex flex-wrap justify-center gap-3">
              <NuxtLink to="/how-to" class="btn btn-primary">Read the how-to</NuxtLink>
              <a :href="repo" rel="noopener" class="btn border-white/20 bg-white/10 text-white hover:border-white/40">Star on GitHub</a>
            </div>
          </div>
        </div>
      </div>
    </section>
  </div>
</template>

<style>
.rd-cover {
  color: var(--fg);
  background: var(--surface-2);
}
.rd-cover::after {
  content: '';
  position: absolute;
  inset: 0;
  border-radius: 6px;
  background:
    repeating-linear-gradient(90deg, rgba(255, 255, 255, 0.14) 0 8px, transparent 8px 16px),
    repeating-linear-gradient(0deg, rgba(4, 48, 64, 0.14) 0 8px, transparent 8px 16px), #7fa9b3;
  transform-origin: left;
  transform: scaleX(0);
  transition: transform 0.8s cubic-bezier(0.65, 0, 0.35, 1);
}
.is-in .rd-cover::after {
  transform: scaleX(1);
}
.is-in div:nth-child(2) > .rd-cover::after {
  transition-delay: 0.15s;
}
.is-in div:nth-child(3) > .rd-cover::after {
  transition-delay: 0.3s;
}
.is-in div:nth-child(4) > .rd-cover::after {
  transition-delay: 0.45s;
}
</style>
