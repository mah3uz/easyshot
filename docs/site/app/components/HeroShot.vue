<script setup lang="ts">
// The hero picture: a frozen desktop, a region being selected, and the preview deck.
// Drawn in HTML so it stays sharp, themes itself and animates without video.
const corners = [
  { icon: 'pin', at: 'top-1.5 left-1.5' },
  { icon: 'x', at: 'top-1.5 right-1.5' },
  { icon: 'pencil', at: 'bottom-1.5 left-1.5' },
  { icon: 'cloud-upload', at: 'bottom-1.5 right-1.5' },
]
</script>

<template>
  <div class="relative mx-auto w-full max-w-[1080px] [perspective:2000px]">
    <div class="pointer-events-none absolute -inset-x-10 -top-10 bottom-0 -z-10 rounded-[60px] bg-[radial-gradient(ellipse_at_center,var(--glow),transparent_65%)] blur-2xl" />
    <div class="hero-tilt relative overflow-hidden rounded-card border border-line-strong shadow-float">
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
          <div class="sel-box absolute inset-0 rounded-[3px] border-[1.5px] border-white">
            <i v-for="h in ['-left-1 -top-1','-right-1 -top-1','-left-1 -bottom-1','-right-1 -bottom-1']" :key="h" class="absolute size-2 rounded-full border border-teal-deep bg-white" :class="h" />
          </div>
          <div class="sel-size absolute -bottom-8 left-1/2 -translate-x-1/2 rounded-md bg-teal-ink/85 px-2 py-0.5 font-mono text-[10px] whitespace-nowrap sm:text-[11px] text-white backdrop-blur">
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
  width: 0;
  height: 0;
  box-shadow: 0 0 0 9999px rgba(3, 30, 40, 0.45);
  animation: select 7s cubic-bezier(0.65, 0, 0.35, 1) 1.2s infinite backwards;
}
@media (min-width: 640px) {
  .sel {
    top: 14%;
    left: 20%;
  }
}
.sel-box,
.sel-size {
  opacity: 0;
  animation: show 7s linear 1.2s infinite;
}
@keyframes select {
  0% {
    width: 0;
    height: 0;
    box-shadow: 0 0 0 9999px rgba(3, 30, 40, 0);
  }
  8% {
    width: 0;
    height: 0;
    box-shadow: 0 0 0 9999px rgba(3, 30, 40, 0.45);
  }
  35%,
  70% {
    width: 50%;
    height: 58%;
    box-shadow: 0 0 0 9999px rgba(3, 30, 40, 0.45);
  }
  80%,
  100% {
    width: 50%;
    height: 58%;
    box-shadow: 0 0 0 9999px rgba(3, 30, 40, 0);
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
  .sel {
    animation: none;
    width: 50%;
    height: 58%;
    box-shadow: 0 0 0 9999px rgba(3, 30, 40, 0.45);
  }
  .sel-box,
  .sel-size,
  .deck-card,
  .deck-peek,
  .deck-count {
    opacity: 1;
    transform: none;
  }
}
</style>
