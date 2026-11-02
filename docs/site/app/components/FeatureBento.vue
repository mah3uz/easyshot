<script setup lang="ts">
// "Everything in one app" grid. Each tile carries a tiny picture of the feature.
const tiles = [
  { icon: 'square-dashed', title: 'Capture an area', text: 'The screen freezes, you drag, you adjust. Your last area is already selected.', span: 'md:col-span-2', art: 'area' },
  { icon: 'app-window', title: 'Window capture', text: 'Hover to highlight a window, click to take it.', art: 'window' },
  { icon: 'scan-text', title: 'Text and QR (OCR)', text: 'Copy text out of any picture, with or without line breaks.', art: 'ocr' },
  { icon: 'pipette', title: 'Colour picker', text: 'Copies the colour under the cursor as #RRGGBB.', art: 'color' },
  { icon: 'unfold-vertical', title: 'Scrolling capture', text: 'Scroll a page and get one tall image, up to 30,000 pixels.', art: 'scroll' },
  { icon: 'pin', title: 'Pin to screen', text: 'Keep a screenshot floating above every window while you work.', art: 'pin' },
  { icon: 'timer', title: 'Timed capture', text: 'Wait 3, 5 or 10 seconds to open that menu first.', art: 'timer' },
  { icon: 'video', title: 'Screen recording', text: 'Record an area, a display or a window, with system audio and your microphone.', span: 'md:col-span-2', art: 'rec' },
  { icon: 'images', title: 'Preview deck', text: 'Every capture lands as a card. Drag it into any app, or save, copy and annotate it.', span: 'md:col-span-2', art: 'deck' },
]
</script>

<template>
  <div class="grid gap-4 sm:grid-cols-2 md:grid-cols-4">
    <article
      v-for="(t, i) in tiles"
      :key="t.title"
      v-reveal="(i % 4) * 0.07"
      class="card card-hover group relative flex min-h-[230px] flex-col justify-between overflow-hidden p-6"
      :class="t.span"
    >
      <div class="pointer-events-none relative mb-5 h-[92px]" aria-hidden="true">
        <!-- area -->
        <div v-if="t.art === 'area'" class="absolute inset-0 overflow-hidden rounded-xl bg-[url(/img/landscape.svg)] bg-cover">
          <img :src="photoUrl('valley', 640)" alt="" class="absolute inset-0 size-full object-cover" loading="lazy" width="640" height="360" onerror="this.style.display='none'" />
          <div class="absolute inset-[18%_30%_18%_22%] rounded border-[1.5px] border-white shadow-[0_0_0_999px_rgba(3,30,40,.35)] transition-all duration-700 ease-out-soft group-hover:inset-[12%_18%_12%_14%]" />
          <span class="absolute right-3 bottom-2 rounded bg-teal-ink/80 px-1.5 font-mono text-[10px] text-white">640 × 360</span>
        </div>
        <!-- window -->
        <div v-else-if="t.art === 'window'" class="absolute inset-0 grid place-items-center">
          <div class="h-16 w-28 rounded-lg border border-line bg-surface-2 shadow-card ring-0 ring-accent transition-all duration-500 group-hover:ring-4">
            <div class="flex gap-1 p-1.5"><i class="size-1.5 rounded-full bg-[#ff5f57]" /><i class="size-1.5 rounded-full bg-[#febc2e]" /><i class="size-1.5 rounded-full bg-[#28c840]" /></div>
          </div>
        </div>
        <!-- ocr -->
        <div v-else-if="t.art === 'ocr'" class="absolute inset-0 grid content-center gap-1.5 font-mono text-[11px]">
          <span class="w-fit rounded bg-accent-soft px-1.5 text-accent transition-all duration-500 group-hover:bg-accent group-hover:text-[#032830]">Card 4242 1234</span>
          <span class="w-fit rounded px-1.5 text-fg-3">Copied to clipboard ✓</span>
        </div>
        <!-- colour -->
        <div v-else-if="t.art === 'color'" class="absolute inset-0 flex items-center gap-3">
          <span class="size-14 rounded-2xl bg-[#21bdb0] shadow-card transition-transform duration-500 group-hover:scale-110 group-hover:rotate-6" />
          <span class="font-mono text-sm font-medium">#21BDB0</span>
        </div>
        <!-- scroll -->
        <div v-else-if="t.art === 'scroll'" class="absolute inset-0 overflow-hidden rounded-xl border border-line bg-surface-2">
          <div class="grid gap-1.5 p-3 transition-transform duration-[1.6s] ease-out-soft group-hover:-translate-y-14">
            <i v-for="n in 9" :key="n" class="h-2 rounded-full bg-line-strong" :style="{ width: `${50 + ((n * 23) % 45)}%` }" />
          </div>
        </div>
        <!-- pin -->
        <div v-else-if="t.art === 'pin'" class="absolute inset-0 grid place-items-center">
          <img :src="photoUrl('fog', 400)" alt="" class="aspect-[16/10] w-28 rounded-lg bg-[url(/img/landscape.svg)] bg-cover object-cover shadow-card ring-2 ring-white transition-transform duration-500 ease-out-soft group-hover:-translate-y-1.5 group-hover:-rotate-3" loading="lazy" width="400" height="250" onerror="this.onerror=null;this.src='/img/landscape.svg'" />
        </div>
        <!-- timer -->
        <div v-else-if="t.art === 'timer'" class="absolute inset-0 flex items-center gap-2">
          <span v-for="n in [3, 5, 10]" :key="n" class="grid size-12 place-items-center rounded-full border border-line font-display text-lg font-bold transition-colors duration-300 group-hover:border-accent group-hover:text-accent">{{ n }}</span>
        </div>
        <!-- rec -->
        <div v-else-if="t.art === 'rec'" class="absolute inset-0 overflow-hidden rounded-xl bg-[url(/img/wall-2.webp)] bg-cover">
          <div class="absolute inset-[14%_26%_30%_26%] rounded-md border-2 border-[#ff4f6d] shadow-[0_0_0_999px_rgba(3,30,40,.35)]" />
          <div class="absolute bottom-2 left-1/2 flex -translate-x-1/2 items-center gap-1.5 rounded-full bg-teal-ink/85 px-2 py-1 text-[10px] text-white">
            <span class="size-2 animate-pulse rounded-[2px] bg-[#ff4f6d]" /><span class="font-mono">00:12</span>
          </div>
        </div>
        <!-- deck -->
        <div v-else-if="t.art === 'deck'" class="absolute inset-0 flex items-end gap-2">
          <img v-for="(k, n) in (['forest', 'lake', 'valley'] as const)" :key="k" :src="photoUrl(k, 400)" alt="" loading="lazy" width="400" height="250" onerror="this.onerror=null;this.src='/img/landscape.svg'" class="aspect-[16/10] w-[30%] max-w-28 rounded-lg border border-line bg-[url(/img/landscape.svg)] bg-cover object-cover shadow-card transition-transform duration-500 ease-out-soft group-hover:-translate-y-2" :style="{ transitionDelay: `${n * 60}ms` }" />
        </div>
      </div>
      <div class="grid gap-1.5">
        <h3 class="flex items-center gap-2 text-lg"><Icon :name="t.icon" :size="18" class="text-accent" />{{ t.title }}</h3>
        <p class="text-[14.5px] leading-relaxed text-fg-2">{{ t.text }}</p>
      </div>
    </article>
  </div>
</template>
