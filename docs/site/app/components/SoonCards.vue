<script setup lang="ts">
const studio = [
  'A timeline to cut, split and speed up',
  'Automatic zoom that follows your clicks',
  'Smooth pointer movement',
  'Your camera in a corner or side by side',
  'Captions from speech',
  'Replace or clean up the audio',
  'Backgrounds and style presets',
  'Reframe for 16:9, 9:16 or 1:1',
  'AI agents can edit through the same commands',
]

const share = [
  'A free hosted link that expires on its own',
  'Or your own S3-compatible storage (R2, S3, B2, MinIO), with keys kept in your system keyring',
  'A viewer page with a proper link preview',
  'Delete a share whenever you want',
]

const wave = Array.from({ length: 64 }, (_, i) => 22 + Math.round(70 * Math.abs(Math.sin(i * 0.55) * Math.cos(i * 0.17))))
</script>

<template>
  <div class="grid grid-cols-[minmax(0,1fr)] gap-5 lg:grid-cols-[1.2fr_1fr]">
    <article v-reveal class="card grid grid-cols-[minmax(0,1fr)] content-start gap-6 p-6 sm:p-8">
      <div class="flex items-center justify-between gap-4">
        <span class="icon-tile"><Icon name="film" :size="21" /></span>
        <span class="hero-ping inline-flex items-center gap-[7px] rounded-full bg-accent-soft px-2.5 py-[3px] text-[12.5px] font-semibold tracking-[0.02em] whitespace-nowrap text-accent">Coming soon</span>
      </div>
      <div class="grid gap-2">
        <h3 class="text-[clamp(24px,2.6vw,30px)] leading-tight">Video Studio</h3>
        <p class="text-[17px] text-fg-2">When a trim isn't enough: turn a recording into a polished video.</p>
      </div>

      <div class="relative grid h-[172px] grid-cols-[18px_1fr] content-center gap-x-3 gap-y-2.5 rounded-card-sm border border-line bg-surface-2 px-3.5 text-fg-3" aria-hidden="true">
        <span />
        <div class="tl-ruler h-2.5" />
        <Icon name="film" :size="14" class="self-center" />
        <div class="relative h-9">
          <span class="tl-clip left-0 w-[37%]">Intro</span>
          <span class="tl-clip left-[38.5%] w-[29%]">Demo<em>1.5×</em></span>
          <span class="tl-clip left-[69%] w-[31%]">Wrap-up</span>
        </div>
        <Icon name="target" :size="14" class="self-center" />
        <div class="relative h-6">
          <span class="absolute inset-y-0 left-[44%] flex w-[20%] items-center gap-1 overflow-hidden rounded-[5px] border border-dashed border-accent bg-accent-soft px-1.5 font-mono text-[10.5px] font-semibold whitespace-nowrap text-accent">
            <Icon name="mouse-pointer-2" :size="11" />Zoom 2×
          </span>
        </div>
        <Icon name="volume-2" :size="14" class="self-center" />
        <div class="flex h-6 items-center gap-[2px] overflow-hidden">
          <i v-for="(h, i) in wave" :key="i" class="w-full min-w-px rounded-full bg-accent-2/45" :style="{ height: `${h}%` }" />
        </div>
        <div class="@container pointer-events-none absolute inset-y-3 right-3.5 left-[44px]">
          <span class="tl-head absolute inset-y-0 w-0.5 rounded-full bg-[#ff4f6d]" />
        </div>
      </div>

      <ul class="grid gap-x-6 gap-y-2.5 sm:grid-cols-2">
        <li v-for="p in studio" :key="p" class="flex gap-3 text-[15px] text-fg-2">
          <span class="mt-0.5 grid size-5 shrink-0 place-items-center rounded-full bg-accent-soft"><i class="size-1.5 rounded-full bg-accent" /></span>
          {{ p }}
        </li>
      </ul>
    </article>

    <article v-reveal="0.1" class="card grid grid-cols-[minmax(0,1fr)] content-start gap-6 p-6 sm:p-8">
      <div class="flex items-center justify-between gap-4">
        <span class="icon-tile"><Icon name="cloud-upload" :size="21" /></span>
        <span class="hero-ping inline-flex items-center gap-[7px] rounded-full bg-accent-soft px-2.5 py-[3px] text-[12.5px] font-semibold tracking-[0.02em] whitespace-nowrap text-accent">Coming soon</span>
      </div>
      <div class="grid gap-2">
        <h3 class="text-[clamp(24px,2.6vw,30px)] leading-tight">Cloud sharing</h3>
        <p class="text-[17px] text-fg-2">Share a capture as a link.</p>
      </div>

      <div class="grid h-[172px] content-center gap-3 rounded-card-sm border border-line bg-surface-2 px-3.5" aria-hidden="true">
        <div class="flex h-10 items-center gap-2 rounded-full border border-line-strong bg-surface pr-1 pl-3.5 shadow-soft">
          <Icon name="link" :size="15" class="shrink-0 text-accent" />
          <span class="min-w-0 flex-1 truncate font-mono text-[13px] text-fg"><span class="text-fg-3">https://…/s/</span>k7Qm2xP4</span>
          <span class="share-copy relative grid h-8 w-[84px] shrink-0 rounded-full text-[12.5px] font-semibold text-[#032830]" style="background: var(--grad)">
            <span class="flex items-center justify-center gap-1.5 [grid-area:1/1]"><Icon name="copy" :size="13" />Copy</span>
            <span class="flex items-center justify-center gap-1.5 [grid-area:1/1]"><Icon name="check" :size="13" />Copied</span>
          </span>
        </div>
        <div class="flex items-center gap-3 rounded-card-sm border border-line border-l-[3px] border-l-accent bg-surface p-2.5 shadow-soft">
          <img :src="photoUrl('lake', 400)" alt="" width="80" height="50" loading="lazy" class="h-[50px] w-20 shrink-0 rounded-card-xs bg-[url(/img/landscape.svg)] bg-cover object-cover" />
          <span class="grid min-w-0 flex-1 gap-0.5">
            <b class="truncate text-[13.5px] font-semibold text-fg">Mountain lake.png</b>
            <span class="truncate text-[12px] text-fg-3">Shared with easyshot</span>
          </span>
          <span class="grid size-8 shrink-0 place-items-center rounded-card-xs text-fg-3"><Icon name="trash" :size="15" /></span>
        </div>
      </div>

      <ul class="grid gap-y-2.5">
        <li v-for="p in share" :key="p" class="flex gap-3 text-[15px] text-fg-2">
          <span class="mt-0.5 grid size-5 shrink-0 place-items-center rounded-full bg-accent-soft"><i class="size-1.5 rounded-full bg-accent" /></span>
          {{ p }}
        </li>
      </ul>
    </article>
  </div>
</template>

<style scoped>
.tl-ruler {
  background: repeating-linear-gradient(90deg, var(--line-strong) 0 1px, transparent 1px 10%) bottom / 100% 100% no-repeat,
    repeating-linear-gradient(90deg, var(--line) 0 1px, transparent 1px 2.5%) bottom / 100% 50% no-repeat;
}
.tl-clip {
  position: absolute;
  inset-block: 0;
  display: flex;
  align-items: center;
  gap: 6px;
  overflow: hidden;
  padding-inline: 8px;
  border: 1px solid color-mix(in srgb, var(--accent) 45%, transparent);
  border-radius: 6px;
  background: linear-gradient(180deg, color-mix(in srgb, var(--accent) 30%, transparent), color-mix(in srgb, var(--accent-2) 20%, transparent));
  color: var(--fg);
  font: 500 11px/1 var(--font-mono);
  white-space: nowrap;
}
.tl-clip em {
  padding: 2px 4px;
  border-radius: 4px;
  background: var(--surface);
  color: var(--accent);
  font-style: normal;
  font-size: 10px;
}
.tl-head {
  left: 0;
  animation: tl-play 9s linear infinite;
}
.tl-head::before {
  content: '';
  position: absolute;
  top: -4px;
  left: 50%;
  width: 10px;
  height: 10px;
  border-radius: 3px 3px 50% 50%;
  background: inherit;
  transform: translateX(-50%);
}
@keyframes tl-play {
  from {
    transform: translateX(0);
  }
  to {
    transform: translateX(100cqw);
  }
}
.share-copy > span {
  animation: copy-a 5s ease-in-out infinite;
}
.share-copy > span + span {
  animation-name: copy-b;
}
@keyframes copy-a {
  0%, 58%, 100% {
    opacity: 1;
  }
  60%, 97% {
    opacity: 0;
  }
}
@keyframes copy-b {
  0%, 58%, 100% {
    opacity: 0;
  }
  60%, 97% {
    opacity: 1;
  }
}
</style>
