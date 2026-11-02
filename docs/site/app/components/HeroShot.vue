<script setup lang="ts">
// The hero picture: a frozen desktop, a region being selected, and the preview deck.
// Drawn in HTML so it stays sharp, themes itself and animates without video.
</script>

<template>
  <div class="relative mx-auto w-full max-w-[1080px] [perspective:2000px]">
    <div class="pointer-events-none absolute -inset-x-10 -top-10 bottom-0 -z-10 rounded-[60px] bg-[radial-gradient(ellipse_at_center,var(--glow),transparent_65%)] blur-2xl" />
    <div class="hero-tilt relative overflow-hidden rounded-[22px] border border-line-strong shadow-float sm:rounded-[28px]">
      <img src="/img/wall-3.webp" alt="" class="absolute inset-0 size-full object-cover" width="1600" height="900" fetchpriority="high" />

      <!-- Top bar, like a tiling compositor's panel -->
      <div class="relative flex h-7 items-center justify-between bg-black/25 px-4 text-[11px] font-medium text-white/80 backdrop-blur-md">
        <div class="flex items-center gap-3"><span class="rounded bg-white/25 px-1.5">1</span><span>2</span><span>3</span></div>
        <span class="hidden sm:block">Mon 2 Nov · 10:42</span>
        <div class="flex items-center gap-2"><img src="/logo.svg" alt="" class="size-3.5" /><span>100%</span></div>
      </div>

      <div class="relative aspect-[4/3] w-full sm:aspect-[16/9.4]">
        <!-- A window on the desktop -->
        <div class="absolute top-[12%] left-[8%] w-[80%] sm:top-[9%] sm:left-[16%] sm:w-[58%] overflow-hidden rounded-xl bg-white/95 shadow-2xl">
          <img src="/img/practice.webp" alt="" class="w-full" width="960" height="600" />
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

        <!-- Preview deck -->
        <div class="absolute bottom-[5%] left-[3%] grid w-[22%] min-w-[120px] gap-2">
          <div class="deck-card overflow-hidden rounded-xl border border-white/30 bg-white/20 p-1 shadow-xl backdrop-blur-md">
            <img src="/img/practice.webp" alt="" class="rounded-lg" width="960" height="600" />
            <div class="deck-actions absolute inset-1 flex items-center justify-center gap-1.5 rounded-lg bg-teal-ink/55 opacity-0 backdrop-blur-[2px]">
              <span class="rounded-md bg-white/90 px-2 py-1 text-[10px] font-semibold text-teal-ink">Copy</span>
              <span class="rounded-md bg-white/90 px-2 py-1 text-[10px] font-semibold text-teal-ink">Save</span>
            </div>
          </div>
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
  animation: select 7s cubic-bezier(0.65, 0, 0.35, 1) 1.2s infinite;
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
    width: 50%;
    height: 58%;
    box-shadow: 0 0 0 9999px rgba(3, 30, 40, 0.45);
  }
  .sel-box,
  .sel-size,
  .deck-card {
    opacity: 1;
    transform: none;
  }
}
</style>
