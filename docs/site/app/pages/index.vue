<script setup lang="ts">
const { repo, siteUrl } = useRuntimeConfig().public
const { playing, play } = useTour()

function watchIntro() {
  document.getElementById('tour')?.scrollIntoView({ behavior: 'smooth', block: 'center' })
  play()
}

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
    a: 'On Arch, install <code>easyshot-bin</code> (prebuilt) or <code>easyshot</code> (built from source) from the AUR. On other distributions, download the prebuilt tarball from the <a class="text-accent underline" href="https://github.com/mah3uz/easyshot/releases/latest">releases page</a> and run its <code>install.sh</code>. You can also build it with <code>cargo build --release -p easyshot</code>. See <a class="text-accent underline" href="/#install">Install</a>.',
  },
  {
    q: 'How do I set the keyboard shortcuts?',
    a: 'Wayland apps cannot grab global keys, and easyshot sets none until you choose them. Pick your keys in Settings › Shortcuts or in the first-run step, which suggests the Print keys where they are free. easyshot writes them to its own binds file for Hyprland, sway or niri, and you add the one include line it shows you to your config, once; Check tells you whether it is included. easyshot never edits your config itself.',
  },
  {
    q: 'Does it upload my screenshots?',
    a: 'Only the ones you share. Captures, recordings, logs and crash reports stay on your machine. When you press Share, that one file goes to easyshot Free, or to your own bucket once you have set it up. See <a class="text-accent underline" href="/#share">Share</a>.',
  },
  {
    q: 'Are shared links private?',
    a: 'They are unlisted, not private. A link is long and random and is not listed anywhere, but anyone who has it can open the page and download the file. Share only what you are happy for anyone with the link to see.',
  },
  {
    q: 'How long do shared links last?',
    a: 'On easyshot Free, 1, 3 or 7 days: you choose when you share. Then the file is deleted, and anyone opening the link sees a plain “page not found”; only you see Expired, in the gallery. Links to your own bucket last as long as you keep the files there.',
  },
  {
    q: 'Can I use my own storage?',
    a: 'Yes. In Settings › Sharing, fill in the endpoint, bucket and public URL of any S3-compatible storage, such as Cloudflare R2, AWS S3, Backblaze B2 or MinIO. From then on easyshot uploads straight to it; until then it uses easyshot Free. Your keys stay in your keyring, or in a private file when no keyring is running.',
  },
  {
    q: 'What are the limits on free sharing?',
    a: 'Images up to 10 MB, GIFs up to 20 MB and videos up to 200 MB. easyshot re-encodes a bigger video to fit, which holds roughly 13 to 27 minutes of 1080p screen recording. Each install also has a daily allowance. See the <a class="text-accent underline" href="/terms">Terms of Service</a>.',
  },
  {
    q: 'What does recording need?',
    a: "gpu-screen-recorder (preferred) or wf-recorder, and ffmpeg to trim and export. Without a recorder, the tray's Record item is greyed out and tells you what to install. Recording a window from its own picture needs GStreamer's good plugins and an H.264 encoder; without them a window recording records the window's area on screen. Text capture needs Tesseract; QR codes are read without it.",
  },
  {
    q: 'Can I edit recordings into a video?',
    a: 'Yes, in the <a class="text-accent underline" href="/#trim">trim viewer</a>: trim the start and end, set the sound and the look, and export MP4, WebM or GIF. A full Video Studio is <a class="text-accent underline" href="/#soon">coming soon</a>.',
  },
  {
    q: 'Can AI agents use it?',
    a: 'Yes, once you allow it, through the command line, a skill or an MCP server. Agent access is off until you turn it on, and asks you in a dialog first. See <a class="text-accent underline" href="/#agents">AI agents</a>.',
  },
]

const why = [
  { icon: 'heart', title: 'Free and open source', text: 'Read the code, change it, ship it.' },
  { icon: 'shield-check', title: 'Private by default', text: 'No account. Nothing leaves your machine unless you share it.' },
  { icon: 'zap', title: 'Native and fast', text: 'Written in Rust with GPUI, rendered on your GPU.' },
  { icon: 'keyboard', title: 'Keyboard first', text: 'Every action has a command, and your compositor binds the keys.' },
]

const chips = [
  { icon: 'square-dashed', text: 'Area & window' },
  { icon: 'unfold-vertical', text: 'Scrolling capture' },
  { icon: 'scan-text', text: 'Text (OCR)' },
  { icon: 'pencil-line', text: 'Annotate' },
  { icon: 'eye-off', text: 'Smart Redaction' },
  { icon: 'video', text: 'Screen recording' },
  { icon: 'cloud-upload', text: 'Share a link' },
  { icon: 'pin', text: 'Pin to screen' },
  { icon: 'sparkles', text: 'AI agents (MCP)' },
]

const editorPoints = [
  'Arrows, shapes, pen, highlighter, text, numbered steps, blur and pixelate',
  'Crop with aspect presets, rotate and flip, and undo for all of it',
  'Background, padding, corners, shadow, border, watermark and presets',
  'Camera 3D and progressive blur for a polished look',
  'Unsaved edits come back after a crash',
]

const trimPoints = [
  'Trim the start and the end',
  'A volume slider each for system sound and your microphone, mixed into one track on export',
  'Brightness and four colour looks (Natural, Vivid, Muted and Mono), shown in the player as you choose',
  'MP4, WebM or GIF, at the original size or 1080p, 720p or 480p, in High, Balanced or Small quality',
  'Your last choices are remembered',
  'Saved to <code>~/Videos/easyshot</code>, never over an earlier file',
]

const sharePoints = [
  'A toast shows the upload with Cancel, then Open and Copy Link',
  'The link opens a page with the image or video, Download and Copy link',
  'In the gallery, a cloud mark shows the time left, Cloud Shares lists every link, and Delete Cloud Share takes one down',
  'Unlisted, not private: anyone who has the link can open it',
  'From a terminal: <code>easyshot share shot.png</code>',
]

const shareTargets = [
  {
    icon: 'cloud-upload',
    title: 'easyshot Free',
    text: 'The default: no account and no setup.',
    points: [
      'Images up to 10 MB, GIFs up to 20 MB',
      'Videos up to 200 MB, re-encoded to fit: roughly 13 to 27 minutes of 1080p screen recording',
      'A daily allowance for each install',
      'After the days you pick, the file is deleted and the link shows “page not found”',
    ],
  },
  {
    icon: 'hard-drive',
    title: 'Your own bucket',
    text: 'Used once you set it up in Settings: any S3-compatible storage, such as Cloudflare R2, AWS S3, Backblaze B2 or MinIO.',
    points: [
      'easyshot uploads straight to your storage',
      'Keys in your keyring, or in a private file when no keyring runs',
      'Links last as long as you keep the files',
      'Test Connection in Settings › Sharing checks it end to end',
    ],
  },
]

const agentSafety = [
  'Off until you turn it on, in Settings › Agents or with <code>easyshot agent enable</code>',
  'A dialog asks you first: once per session by default, or every time',
  'Answers only your own user, and nothing goes over the network',
  'A tray badge shows while access is granted, with a one-click Revoke',
  'An agent’s recording stops, and is kept, when its access ends',
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
          '@type': 'VideoObject',
          name: tour.title,
          description: tour.description,
          thumbnailUrl: `${siteUrl}/img/tour/poster-1920.webp`,
          uploadDate: tour.uploadDate,
          duration: tour.duration,
          embedUrl: tourEmbed,
          url: `https://www.youtube.com/watch?v=${tour.id}`,
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

      <div class="container-x grid justify-items-center gap-6 text-center">
        <div class="hero-mark rise">
          <img src="/logo.svg" alt="easyshot logo" width="112" height="112" class="size-full animate-float drop-shadow-[0_22px_30px_rgba(19,136,166,.35)]" />
        </div>
        <a :href="repo" rel="noopener" class="rise group inline-flex items-center gap-2.5 rounded-full border border-line bg-surface py-1.5 pr-4 pl-2 text-sm text-fg-2 hover-motion hover:border-accent max-sm:gap-2 max-sm:pr-3 max-sm:text-[12.5px]" style="--d: 0.08s">
          <b class="hero-ping inline-flex shrink-0 items-center gap-[7px] rounded-full bg-accent-soft px-2.5 py-[3px] text-[12.5px] font-semibold tracking-[0.02em] whitespace-nowrap text-accent">Open source</b>
          Free, built with Rust
          <Icon name="arrow-up-right" :size="14" class="hover-motion group-hover:translate-x-0.5 group-hover:-translate-y-0.5" />
        </a>
        <h1 class="rise max-w-[820px] text-[clamp(42px,8vw,88px)] leading-[1.02] font-extrabold" style="--d: 0.16s">
          Screenshots, done right on
          <span class="hero-word"><span class="grad-text">Wayland</span><span class="hero-sel" aria-hidden="true"><i /><i /><i /><i /><em>812 × 214</em></span></span>.
        </h1>
        <p class="rise mt-[18px] max-w-[600px] text-[clamp(17px,2.3vw,21px)] leading-relaxed text-pretty text-fg-2 max-sm:mt-2.5" style="--d: 0.24s">
          Capture an area, a window, text or a scrolling page, mark it up in a full editor, and record your screen. All
          from a key bind, on Hyprland, sway and niri.
        </p>
        <ul class="rise flex max-w-[760px] flex-wrap justify-center gap-2" style="--d: 0.32s" aria-label="Features">
          <li v-for="c in chips" :key="c.text" class="hero-chip inline-flex items-center gap-2 rounded-full border border-line bg-surface px-3.5 py-2 text-sm text-fg-2 hover:border-accent hover:text-fg">
            <Icon :name="c.icon" :size="16" class="text-accent" />{{ c.text }}
          </li>
        </ul>
        <div class="rise mt-2 flex flex-wrap justify-center gap-3 max-sm:w-full max-sm:max-w-[340px]" style="--d: 0.4s">
          <NuxtLink to="/how-to" class="btn btn-primary max-sm:w-full">
            <Icon name="download" :size="18" /> Get started
          </NuxtLink>
          <a :href="repo" rel="noopener" class="btn btn-ghost max-sm:w-full">
            <svg width="18" height="18" viewBox="0 0 24 24" fill="currentColor" aria-hidden="true"><path d="M12 .5a11.5 11.5 0 0 0-3.64 22.41c.58.1.79-.25.79-.56v-2c-3.2.7-3.88-1.37-3.88-1.37-.53-1.33-1.28-1.69-1.28-1.69-1.05-.72.08-.7.08-.7 1.16.08 1.77 1.19 1.77 1.19 1.03 1.77 2.7 1.26 3.36.96.1-.75.4-1.26.73-1.55-2.55-.29-5.24-1.28-5.24-5.69 0-1.26.45-2.29 1.19-3.1-.12-.29-.52-1.46.11-3.05 0 0 .97-.31 3.17 1.18a11 11 0 0 1 5.77 0c2.2-1.49 3.17-1.18 3.17-1.18.63 1.59.23 2.76.11 3.05.74.81 1.19 1.84 1.19 3.1 0 4.42-2.7 5.39-5.26 5.68.41.36.78 1.06.78 2.14v3.17c0 .31.21.67.8.56A11.5 11.5 0 0 0 12 .5Z" /></svg>
            View on GitHub
          </a>
        </div>
        <HeroInstall class="rise" style="--d: 0.48s" />
        <div class="rise mt-1.5 flex flex-wrap items-center justify-center gap-x-7 gap-y-2.5 text-[13px] text-fg-3" style="--d: 0.56s" aria-label="Supported compositors">
          <span class="max-sm:basis-full">Made for tiling Wayland</span>
          <b v-for="c in ['Hyprland', 'sway', 'niri']" :key="c" class="font-display text-xl font-bold tracking-[-0.03em] text-fg-2">{{ c }}</b>
        </div>
      </div>

      <div class="container-x mt-16 sm:mt-20">
        <HeroShot />
        <div class="mt-10 grid justify-items-center gap-5 text-center sm:mt-12">
          <p class="max-w-[780px] font-display text-[clamp(20px,2.6vw,26px)] leading-snug font-semibold tracking-[-0.02em] text-balance text-fg">
            From the first key to the last export: <span class="grad-text">one afternoon with easyshot.</span>
          </p>
          <button v-if="!playing" type="button" class="btn btn-ghost max-sm:w-full max-sm:max-w-[340px]" @click="watchIntro">
            <Icon name="circle-play" :size="18" class="text-accent" /> Watch the intro
          </button>
        </div>
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
            <li class="flex items-center gap-3"><Icon name="film" :size="17" class="text-accent" /> <a href="#trim" class="underline decoration-line-strong underline-offset-4 hover-motion hover:text-accent">Trim viewer with MP4, WebM and GIF export</a></li>
          </ul>
        </div>
        <div v-reveal="0.1"><RecordMock /></div>
      </div>
    </section>

    <!-- Trim viewer -->
    <section id="trim" class="py-24 sm:py-32">
      <div class="container-x grid items-center gap-12 md:grid-cols-[1.1fr_1fr]">
        <div v-reveal class="order-2 md:order-1"><TrimMock /></div>
        <div v-reveal="0.1" class="order-1 grid gap-4 md:order-2 [&_code]:rounded-md [&_code]:border [&_code]:border-line [&_code]:bg-surface-2 [&_code]:px-1.5 [&_code]:py-px [&_code]:text-[0.88em] [&_code]:whitespace-nowrap [&_code]:text-fg">
          <span class="eyebrow"><Icon name="film" :size="15" />Edit your recording</span>
          <h2 class="text-[clamp(32px,4.4vw,50px)] leading-[1.05]">Trim it, tune it, export it.</h2>
          <p class="text-lg text-fg-2">
            The trim viewer opens from a recording's card, from the gallery, or with <code>easyshot open video.mp4</code>.
            Cut the ends, set the sound, pick a look, and export.
          </p>
          <ul class="grid gap-2.5 pt-2">
            <li v-for="(p, i) in trimPoints" :key="p" v-reveal="i * 0.05" class="flex gap-3 text-[15px] text-fg-2">
              <span class="mt-0.5 grid size-5 shrink-0 place-items-center rounded-full bg-accent-soft text-accent"><Icon name="check" :size="12" /></span>
              <span v-html="p" />
            </li>
          </ul>
        </div>
      </div>
      <div id="soon" class="container-x mt-16 scroll-mt-28 sm:mt-20"><SoonCards /></div>
    </section>

    <!-- Share -->
    <section id="share" class="bg-bg-2/60 py-24 sm:py-32">
      <div class="container-x grid grid-cols-[minmax(0,1fr)] gap-12">
        <div class="grid items-center gap-12 md:grid-cols-[1fr_1.1fr]">
          <div v-reveal class="grid gap-4 [&_code]:rounded-md [&_code]:border [&_code]:border-line [&_code]:bg-surface-2 [&_code]:px-1.5 [&_code]:py-px [&_code]:text-[0.88em] [&_code]:whitespace-nowrap [&_code]:text-fg">
            <span class="eyebrow"><Icon name="cloud-upload" :size="15" />Share</span>
            <h2 class="text-[clamp(32px,4.4vw,50px)] leading-[1.05]">Send it as a link.</h2>
            <p class="text-lg text-fg-2">
              Press Share on a capture card, in the editor or in the trim viewer, and pick 1, 3 or 7 days for easyshot Free. The link
              is on your clipboard at once, so you can paste it while the file uploads.
            </p>
            <ul class="grid gap-2.5 pt-2">
              <li v-for="(p, i) in sharePoints" :key="p" v-reveal="i * 0.05" class="flex gap-3 text-[15px] text-fg-2">
                <span class="mt-0.5 grid size-5 shrink-0 place-items-center rounded-full bg-accent-soft text-accent"><Icon name="check" :size="12" /></span>
                <span v-html="p" />
              </li>
            </ul>
          </div>
          <div v-reveal="0.1"><ShareMock /></div>
        </div>
        <div class="grid gap-4 md:grid-cols-2">
          <article v-for="(t, i) in shareTargets" :key="t.title" v-reveal="i * 0.07" class="card grid content-start gap-4 p-6 sm:p-8">
            <span class="icon-tile"><Icon :name="t.icon" :size="21" /></span>
            <div class="grid gap-1.5">
              <h3 class="text-[clamp(20px,2.2vw,24px)] leading-tight">{{ t.title }}</h3>
              <p class="text-[15px] text-fg-2">{{ t.text }}</p>
            </div>
            <ul class="grid gap-2.5">
              <li v-for="p in t.points" :key="p" class="flex gap-3 text-[15px] text-fg-2">
                <span class="mt-0.5 grid size-5 shrink-0 place-items-center rounded-full bg-accent-soft"><i class="size-1.5 rounded-full bg-accent" /></span>
                {{ p }}
              </li>
            </ul>
          </article>
        </div>
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

    <!-- AI agents -->
    <section id="agents" class="bg-bg-2/60 py-24 sm:py-32">
      <div class="container-x grid items-start gap-12 md:grid-cols-[1fr_1.1fr]">
        <div v-reveal class="grid gap-4 [&_code]:rounded-md [&_code]:border [&_code]:border-line [&_code]:bg-surface-2 [&_code]:px-1.5 [&_code]:py-px [&_code]:text-[0.88em] [&_code]:whitespace-nowrap [&_code]:text-fg">
          <span class="eyebrow"><Icon name="sparkles" :size="15" />AI agents</span>
          <h2 class="text-[clamp(32px,4.4vw,50px)] leading-[1.05]">Let your agent<br />see your screen.</h2>
          <p class="text-lg text-fg-2">
            <code>easyshot agent</code> (or <code>easyshot ai</code>) lets AI agents and scripts list your windows, take
            screenshots with secrets covered, read text and QR codes, annotate, and record and export short demos. Every
            command can answer in JSON, with a clear exit code for each kind of error.
          </p>
          <ul class="grid gap-3 pt-2">
            <li v-for="(p, i) in agentSafety" :key="p" v-reveal="i * 0.05" class="flex gap-3 text-[15px] text-fg-2">
              <span class="mt-0.5 grid size-5 shrink-0 place-items-center rounded-full bg-accent-soft text-accent"><Icon name="check" :size="12" /></span>
              <span v-html="p" />
            </li>
          </ul>
        </div>
        <div v-reveal="0.1"><AgentTabs /></div>
      </div>
    </section>

    <!-- Why -->
    <section class="py-24 sm:py-32">
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
    <section id="install" class="bg-bg-2/60 py-24 sm:py-32">
      <div class="container-x grid items-start gap-12 md:grid-cols-[0.9fr_1.1fr]">
        <div v-reveal class="grid gap-4">
          <span class="eyebrow"><Icon name="download" :size="15" />Install</span>
          <h2 class="text-[clamp(32px,4.4vw,50px)] leading-[1.05]">Get it your way.</h2>
          <p class="text-lg text-fg-2">
            easyshot comes as two Arch (AUR) packages, as a prebuilt tarball with an install script for other distributions, or you can build it from source. The package starts the background service
            for you. Then choose your keys and press one.
          </p>
          <NuxtLink to="/how-to" class="group mt-2 inline-flex w-fit items-center gap-1.5 font-semibold text-accent">
            Full setup guide <Icon name="arrow-up-right" :size="16" class="hover-motion group-hover:translate-x-0.5 group-hover:-translate-y-0.5" />
          </NuxtLink>
        </div>
        <div v-reveal="0.1"><InstallTabs /></div>
      </div>
    </section>

    <!-- FAQ -->
    <section id="faq" class="py-24 sm:py-32">
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
