<script setup lang="ts">
// The hero picture: a frozen desktop, a region being selected, and the preview deck.
// Drawn in HTML so it stays sharp, themes itself and animates without video.
// It is also the tour's poster: nothing is requested from YouTube until it is clicked.
const corners = [
  { icon: 'pin', at: 'top-1.5 left-1.5' },
  { icon: 'x', at: 'top-1.5 right-1.5' },
  { icon: 'pencil', at: 'bottom-1.5 left-1.5' },
  { icon: 'cloud-upload', at: 'bottom-1.5 right-1.5' },
]

const { playing, play } = useTour()
const box = ref<HTMLElement | null>(null)
const frame = ref<HTMLIFrameElement | null>(null)
const height = ref<string>()

// Height cannot ease from auto, so it is pinned in pixels for the move to 16:9, then handed back to aspect-video.
watch(playing, async () => {
  const el = box.value
  if (!el) return
  height.value = `${el.offsetHeight}px`
  await nextTick()
  frame.value?.focus({ preventScroll: true })
  void el.offsetHeight
  height.value = `${(el.offsetWidth * 9) / 16}px`
})
</script>

<template>
  <div id="tour" class="relative mx-auto w-full max-w-[1080px] [perspective:2000px]">
    <div class="pointer-events-none absolute -inset-x-10 -top-10 bottom-0 -z-10 rounded-[60px] bg-[radial-gradient(ellipse_at_center,var(--glow),transparent_65%)] blur-2xl" />
    <div
      ref="box"
      class="hero-tilt relative overflow-hidden rounded-card border border-line-strong shadow-float transition-[height] duration-500 ease-out-soft has-focus-visible:outline-2 has-focus-visible:outline-offset-3 has-focus-visible:outline-accent"
      :class="{ 'bg-black': playing, 'aspect-video': playing && !height }"
      :style="{ height }"
      @transitionend.self="height = undefined"
    >
      <iframe
        v-if="playing"
        ref="frame"
        :src="`${tourEmbed}?autoplay=1&rel=0&cc_load_policy=0`"
        :title="tour.title"
        allow="autoplay; encrypted-media; picture-in-picture; fullscreen"
        allowfullscreen
        class="absolute inset-0 size-full border-0"
      />
      <template v-else>
        <div class="absolute inset-0 bg-[url(/img/wall-3.webp)] bg-cover bg-center" />
        <img
          :src="photoUrl('valley', 1600)"
          :srcset="photoSrcset('valley')"
          sizes="(min-width: 1180px) 1080px, 100vw"
          alt=""
          class="absolute inset-0 size-full object-cover"
          width="1600"
          height="900"
          fetchpriority="high"
         
          onerror="this.style.display='none'"
        />

        <!-- Top bar, like a tiling compositor's panel -->
        <div class="relative flex h-7 items-center justify-between bg-black/25 px-4 text-[11px] font-medium text-white/80 backdrop-blur-md">
          <div class="flex items-center gap-3"><span class="rounded bg-white/25 px-1.5">1</span><span>2</span><span>3</span></div>
          <span class="hidden sm:block">Mon 2 Nov · 10:42</span>
          <div class="flex items-center gap-2"><img src="/logo.svg" alt="" class="size-3.5" /><span>100%</span></div>
        </div>

        <div class="relative aspect-[4/3] w-full sm:aspect-[16/9.4]">
          <!-- A window on the desktop -->
          <div class="absolute top-[10%] left-[8%] w-[80%] shadow-2xl sm:top-[8%] sm:left-[14%] sm:w-[58%]" style="border-radius: 1.6%">
            <ShotWindow photo="lake" eager />
          </div>
          <!-- Second window, a terminal -->
          <div class="absolute top-[22%] right-[5%] hidden w-[26%] overflow-hidden rounded-xl bg-[#062a36]/95 font-mono text-[10px] leading-relaxed text-teal-hi shadow-2xl sm:block">
            <div class="flex gap-1.5 px-3 py-2"><i class="size-2 rounded-full bg-white/20" /><i class="size-2 rounded-full bg-white/20" /><i class="size-2 rounded-full bg-white/20" /></div>
            <div class="px-3 pb-3 text-[#cdeeea]">
              <p><span class="text-teal-hi">$</span> easyshot doctor</p>
              <p class="text-white/50">selection overlay ··· ok</p>
              <p class="text-white/50">window capture ····· ok</p>
              <p class="text-white/50">recording ·········· ok</p>
              <p class="text-white/50">text (OCR) ········· ok</p>
            </div>
          </div>

          <!-- Freeze dim + selection -->
          <div class="sel absolute">
            <div class="sel-dim"><i class="dim-t" /><i class="dim-l" /><i class="dim-r grow" /><i class="dim-b grow" /></div>
            <div class="sel-box absolute inset-0">
              <i v-for="e in ['edge-t', 'edge-l', 'edge-b', 'edge-r']" :key="e" class="grow bg-white" :class="e" />
              <i v-for="h in ['knob-tl', 'knob-tr grow', 'knob-bl grow', 'knob-br grow']" :key="h" class="size-2 rounded-full border border-teal-deep bg-white" :class="h" />
            </div>
            <div class="sel-size absolute rounded-md bg-teal-ink/85 px-2 py-0.5 font-mono text-[10px] whitespace-nowrap sm:text-[11px] text-white">
              812 × 436
            </div>
          </div>

          <!-- Preview deck: stacked in the corner, newest in front, older cards peeking above -->
          <div class="absolute right-[3%] bottom-[5%] w-[24%] min-w-[128px]">
            <div v-for="d in [2, 1]" :key="d" class="deck-peek absolute inset-0 rounded-card-sm border border-white/20 bg-[#0d1b20] shadow-xl" :style="{ '--d': d }" />
            <div class="deck-card relative overflow-hidden rounded-card-sm border border-white/20 bg-black/90 shadow-xl">
              <ShotWindow photo="lake" :chrome="false" class="pointer-events-none" />
              <div class="deck-actions absolute inset-0 bg-black/30 opacity-0">
                <span v-for="c in corners" :key="c.icon" class="absolute grid size-[18px] place-items-center rounded-full border border-white/20 bg-black/45 text-white sm:size-[22px]" :class="c.at">
                  <Icon :name="c.icon" :size="10" />
                </span>
                <div class="flex h-full items-center justify-center gap-1.5">
                  <span v-for="l in ['Copy', 'Save']" :key="l" class="rounded-full bg-white/85 px-2 py-0.5 text-[9px] font-semibold text-black/85 sm:text-[10px]">{{ l }}</span>
                </div>
              </div>
            </div>
            <span class="deck-count absolute -top-[17px] -left-2 grid size-5 place-items-center rounded-full bg-white/95 text-[11px] font-bold text-black/85 shadow-lg">3</span>
          </div>
        </div>

        <button type="button" class="group absolute inset-0 grid cursor-pointer place-items-center hover-motion hover:bg-black/10 focus-visible:bg-black/10 focus-visible:outline-none" aria-label="Play the easyshot tour" @click="play">
          <span class="grid size-14 scale-90 place-items-center rounded-full border border-line-strong bg-surface/70 text-fg opacity-0 shadow-card backdrop-blur-xl hover-motion group-hover:scale-100 group-hover:opacity-100 group-focus-visible:scale-100 group-focus-visible:opacity-100 sm:size-20 [@media(hover:none)]:scale-100 [@media(hover:none)]:opacity-100">
            <svg viewBox="0 0 24 24" class="size-[44%]" fill="currentColor" stroke="currentColor" stroke-width="2" stroke-linejoin="round" aria-hidden="true"><path d="M7.3 6v12l11-6Z" /></svg>
          </span>
        </button>
      </template>
    </div>
  </div>
</template>

<style scoped>
.hero-tilt {
  transform: rotateX(10deg) scale(0.97);
  transform-origin: center top;
  animation: untilt 1.6s cubic-bezier(0.22, 1, 0.36, 1) 0.35s forwards;
}
@keyframes untilt {
  to {
    transform: none;
  }
}
.sel {
  top: 16%;
  left: 12%;
  width: 50%;
  height: 58%;
  container-type: size;
}
@media (min-width: 640px) {
  .sel {
    top: 14%;
    left: 20%;
  }
}
/* The selection grows by transform alone: width, height or a box-shadow would repaint the hero every frame. */
.sel i {
  position: absolute;
  transform-origin: 0 0;
}
.sel-dim {
  animation: dim 7s cubic-bezier(0.65, 0, 0.35, 1) 1.2s infinite backwards;
}
.sel-dim i {
  background: rgba(3, 30, 40, 0.45);
}
.dim-t {
  bottom: 100%;
  left: -9999px;
  right: -9999px;
  height: 9999px;
}
.dim-l {
  top: 0;
  right: 100%;
  width: 9999px;
  height: 9999px;
}
.dim-r {
  top: 0;
  left: 0;
  width: 9999px;
  height: 9999px;
  --from: translateX(0);
  --to: translateX(100cqw);
}
.dim-b {
  top: 0;
  left: 0;
  width: 100%;
  height: 9999px;
  --from: translateY(0) scaleX(0);
  --to: translateY(100cqh) scaleX(1);
}
.edge-t,
.edge-b {
  left: 0;
  width: 100%;
  height: 1.5px;
  --from: translateY(0) scaleX(0);
}
.edge-t {
  top: 0;
  --to: translateY(0) scaleX(1);
}
.edge-b {
  top: -1.5px;
  --to: translateY(100cqh) scaleX(1);
}
.edge-l,
.edge-r {
  top: 0;
  width: 1.5px;
  height: 100%;
  --from: translateX(0) scaleY(0);
}
.edge-l {
  left: 0;
  --to: translateX(0) scaleY(1);
}
.edge-r {
  left: -1.5px;
  --to: translateX(100cqw) scaleY(1);
}
.knob-tl,
.knob-tr,
.knob-bl,
.knob-br {
  top: -4px;
  left: -4px;
  --from: translate(0, 0);
}
.knob-tr {
  --to: translate(100cqw, 0);
}
.knob-bl {
  --to: translate(0, 100cqh);
}
.knob-br {
  --to: translate(100cqw, 100cqh);
}
.grow {
  transform: var(--to);
  animation: grow 7s cubic-bezier(0.65, 0, 0.35, 1) 1.2s infinite backwards;
}
.sel-box,
.sel-size {
  opacity: 0;
  animation: show 7s linear 1.2s infinite;
}
.sel-size {
  top: 0;
  left: 0;
  --from: translate(-50%, calc(32px - 100%));
  --to: translate(calc(50cqw - 50%), calc(100cqh + 32px - 100%));
  transform: var(--to);
  animation:
    show 7s linear 1.2s infinite,
    grow 7s cubic-bezier(0.65, 0, 0.35, 1) 1.2s infinite backwards;
}
@keyframes grow {
  0%,
  8% {
    transform: var(--from);
  }
  35%,
  100% {
    transform: var(--to);
  }
}
@keyframes dim {
  0% {
    opacity: 0;
  }
  8%,
  70% {
    opacity: 1;
  }
  80%,
  100% {
    opacity: 0;
  }
}
@keyframes show {
  0%,
  8% {
    opacity: 0;
  }
  12%,
  72% {
    opacity: 1;
  }
  80%,
  100% {
    opacity: 0;
  }
}
.deck-card {
  position: relative;
  opacity: 0;
  transform: translateX(-30px) scale(0.92);
  animation: deck 7s cubic-bezier(0.22, 1, 0.36, 1) 1.2s infinite;
}
.deck-actions {
  animation: actions 7s ease 1.2s infinite;
}
.deck-peek {
  transform-origin: top center;
  transform: translateY(calc(var(--d) * -7px)) scale(calc(1 - var(--d) * 0.05));
  filter: brightness(calc(1 - var(--d) * 0.2));
  opacity: 0;
  animation: badge 7s linear 1.2s infinite;
}
.deck-count {
  opacity: 0;
  animation: badge 7s linear 1.2s infinite;
}
@keyframes badge {
  0%,
  78% {
    opacity: 0;
  }
  84%,
  97% {
    opacity: 1;
  }
  100% {
    opacity: 0;
  }
}
@keyframes deck {
  0%,
  74% {
    opacity: 0;
    transform: translateX(-30px) scale(0.92);
  }
  82%,
  97% {
    opacity: 1;
    transform: none;
  }
  100% {
    opacity: 0;
    transform: none;
  }
}
@keyframes actions {
  0%,
  86% {
    opacity: 0;
  }
  90%,
  96% {
    opacity: 1;
  }
  100% {
    opacity: 0;
  }
}
@media (prefers-reduced-motion: reduce) {
  .hero-tilt {
    transform: none;
  }
  .sel-dim,
  .grow {
    animation: none;
  }
  .sel-size {
    opacity: 1;
    animation: none;
  }
  .sel-box,
  .deck-card,
  .deck-peek,
  .deck-count {
    opacity: 1;
    transform: none;
  }
}
</style>
