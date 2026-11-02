<script setup lang="ts">
// A working model of the editor's Beautify inspector: the controls change the canvas.
const backgrounds = [
  { id: 'w3', label: 'Dusk wallpaper', css: 'url(/img/wall-3.webp) center/cover' },
  { id: 'w1', label: 'Mist wallpaper', css: 'url(/img/wall-1.webp) center/cover' },
  { id: 'w5', label: 'Ember wallpaper', css: 'url(/img/wall-5.webp) center/cover' },
  { id: 'g1', label: 'Teal gradient', css: 'linear-gradient(135deg,#62edd4,#1388a6)' },
  { id: 'g2', label: 'Sunset gradient', css: 'linear-gradient(135deg,#ffb36b,#ff5e8a 55%,#7a5cff)' },
  { id: 's1', label: 'Plain', css: '#e7eef0' },
]
const bg = ref(backgrounds[0]!)
const padding = ref(9)
const radius = ref(12)
const shadow = ref(true)
const tilt = ref(false)

const tools = ['mouse-pointer-2', 'arrow-up-right', 'rectangle-horizontal', 'pencil-line', 'a-large-small', 'droplet', 'eye-off', 'crop']
const activeTool = ref('arrow-up-right')
</script>

<template>
  <div class="overflow-hidden rounded-[22px] border border-line-strong bg-surface shadow-float">
    <!-- Title bar -->
    <div class="flex h-11 items-center justify-between border-b border-line bg-surface-2 px-3">
      <div class="flex items-center gap-1">
        <button
          v-for="t in tools"
          :key="t"
          type="button"
          class="grid size-8 place-items-center rounded-lg text-fg-2 transition hover:bg-accent-soft hover:text-accent"
          :class="activeTool === t && 'bg-accent-soft text-accent'"
          :aria-label="`Tool: ${t.replace(/-/g, ' ')}`"
          :aria-pressed="activeTool === t"
          @click="activeTool = t"
        >
          <Icon :name="t" :size="16" />
        </button>
      </div>
      <div class="hidden items-center gap-2 sm:flex">
        <span class="rounded-lg px-2.5 py-1 text-xs font-medium text-fg-2">Copy</span>
        <span class="rounded-lg bg-accent px-3 py-1 text-xs font-semibold text-[#032830]">Save</span>
      </div>
    </div>

    <div class="grid md:grid-cols-[1fr_250px]">
      <!-- Canvas -->
      <div class="grid min-h-[280px] place-items-center bg-[repeating-conic-gradient(var(--bg-2)_0_25%,var(--bg)_0_50%)] bg-[length:18px_18px] p-4 sm:p-8">
        <div
          class="w-full max-w-[560px] transition-all duration-700 ease-out-soft [perspective:1200px]"
          :style="{ background: bg.css, padding: `${padding}%`, borderRadius: '14px' }"
        >
          <div
            class="relative overflow-hidden transition-all duration-700 ease-out-soft"
            :style="{
              borderRadius: `${radius}px`,
              boxShadow: shadow ? '0 30px 60px -20px rgba(3,30,40,.55), 0 8px 20px -8px rgba(3,30,40,.35)' : 'none',
              transform: tilt ? 'rotateX(14deg) rotateY(-16deg) rotateZ(3deg)' : 'none',
            }"
          >
            <img src="/img/practice.webp" alt="A screenshot being annotated in the editor" class="w-full" width="960" height="600" loading="lazy" />
            <!-- Annotations -->
            <svg class="absolute inset-0 size-full" viewBox="0 0 960 600" aria-hidden="true">
              <defs>
                <marker id="ah" viewBox="0 0 10 10" refX="6" refY="5" markerWidth="5" markerHeight="5" orient="auto-start-reverse">
                  <path d="M0 0 10 5 0 10z" fill="#ff4f6d" />
                </marker>
              </defs>
              <path class="em-draw" d="M560 470 C 640 520, 700 470, 720 420" fill="none" stroke="#ff4f6d" stroke-width="10" stroke-linecap="round" marker-end="url(#ah)" />
              <rect class="em-draw" x="630" y="190" width="220" height="220" rx="110" fill="none" stroke="#ff4f6d" stroke-width="8" />
            </svg>
            <div class="em-pixelate absolute top-[63.5%] left-[11.5%] h-[11.5%] w-[37%] rounded-md" />
            <span class="absolute top-[82%] left-[44%] rounded-md bg-[#ff4f6d] px-2 py-0.5 text-[clamp(9px,1.6vw,14px)] font-semibold text-white shadow">Ship it!</span>
          </div>
        </div>
      </div>

      <!-- Inspector -->
      <aside class="grid content-start gap-5 border-t border-line p-5 text-sm md:border-t-0 md:border-l" aria-label="Beautify inspector">
        <div class="grid gap-2.5">
          <span class="text-xs font-semibold text-fg-3 uppercase">Background</span>
          <div class="grid grid-cols-6 gap-1.5 md:grid-cols-3">
            <button
              v-for="b in backgrounds"
              :key="b.id"
              type="button"
              class="aspect-[4/3] rounded-lg border-2 transition-all duration-300 hover:scale-105"
              :class="bg.id === b.id ? 'border-accent ring-2 ring-accent-soft' : 'border-transparent'"
              :style="{ background: b.css }"
              :aria-label="b.label"
              :aria-pressed="bg.id === b.id"
              @click="bg = b"
            />
          </div>
        </div>
        <label class="grid gap-2">
          <span class="flex justify-between text-xs font-semibold text-fg-3 uppercase">Padding <span class="font-mono normal-case">{{ padding }}%</span></span>
          <input v-model.number="padding" type="range" min="2" max="16" class="accent-[var(--accent)]" />
        </label>
        <label class="grid gap-2">
          <span class="flex justify-between text-xs font-semibold text-fg-3 uppercase">Corners <span class="font-mono normal-case">{{ radius }}px</span></span>
          <input v-model.number="radius" type="range" min="0" max="28" class="accent-[var(--accent)]" />
        </label>
        <div class="grid grid-cols-2 gap-2">
          <button type="button" class="rounded-lg border px-3 py-2 text-xs font-semibold transition" :class="shadow ? 'border-accent bg-accent-soft text-accent' : 'border-line text-fg-2'" :aria-pressed="shadow" @click="shadow = !shadow">Shadow</button>
          <button type="button" class="rounded-lg border px-3 py-2 text-xs font-semibold transition" :class="tilt ? 'border-accent bg-accent-soft text-accent' : 'border-line text-fg-2'" :aria-pressed="tilt" @click="tilt = !tilt">Camera 3D</button>
        </div>
      </aside>
    </div>
  </div>
</template>

<style>
.em-pixelate {
  background:
    repeating-linear-gradient(90deg, rgba(255, 255, 255, 0.18) 0 10px, transparent 10px 20px),
    repeating-linear-gradient(0deg, rgba(10, 38, 49, 0.12) 0 10px, transparent 10px 20px),
    #c9d4e6;
  backdrop-filter: blur(6px);
}
.em-draw {
  stroke-dasharray: 700;
  stroke-dashoffset: 700;
}
.is-in .em-draw {
  animation: draw 1.6s cubic-bezier(0.65, 0, 0.35, 1) 0.5s forwards;
}
@keyframes draw {
  to {
    stroke-dashoffset: 0;
  }
}
@media (prefers-reduced-motion: reduce) {
  .em-draw {
    stroke-dashoffset: 0;
  }
}
</style>
