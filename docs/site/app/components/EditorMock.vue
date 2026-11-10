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
const radius = ref(6)
const shadow = ref(true)
const tilt = ref(false)

const tools = [
  ['pointer', 'Select'],
  ['rectangle-horizontal', 'Rectangle'],
  ['square', 'Solid rectangle'],
  ['circle', 'Circle'],
  ['slash', 'Straight line'],
  ['arrow-up-right', 'Arrow'],
  ['pencil-line', 'Freehand'],
  ['counter', 'Numbered circle'],
  ['type', 'Text'],
  ['highlight', 'Highlight'],
  ['highlighter', 'Highlighter'],
  ['pixelate', 'Pixelate'],
  ['droplet', 'Blur'],
] as const
const activeTool = ref('Arrow')
const inspector = ref(true)
</script>

<template>
  <div class="overflow-hidden rounded-card border border-line-strong bg-surface shadow-float">
    <!-- Title bar: undo and redo, Save and Export -->
    <div class="flex h-10 items-center justify-between gap-2 border-b border-line bg-surface-2 px-2 sm:px-3">
      <div class="flex items-center gap-0.5 text-fg-2">
        <span class="grid size-7 place-items-center rounded-card-sm" aria-hidden="true"><Icon name="undo-2" :size="15" /></span>
        <span class="grid size-7 place-items-center rounded-card-sm opacity-50" aria-hidden="true"><Icon name="redo-2" :size="15" /></span>
      </div>
      <div class="flex items-center gap-1.5 text-xs font-medium">
        <span class="flex items-center gap-1.5 rounded-card-sm px-2.5 py-1 text-fg-2"><Icon name="download" :size="14" />Save</span>
        <span class="flex items-center gap-1.5 rounded-card-sm bg-accent px-2.5 py-1 font-semibold text-[#032830]"><Icon name="circle-arrow-down" :size="14" />Export</span>
      </div>
    </div>

    <!-- Tool row: crop and rotate, the 13 tools, Smart Redaction, and the inspector toggle -->
    <div class="flex h-11 items-center gap-0.5 overflow-x-auto border-b border-line px-2 text-fg-2 [scrollbar-width:none] sm:px-3" role="toolbar" aria-label="Tools">
      <span class="grid size-8 shrink-0 place-items-center" aria-hidden="true"><Icon name="crop" :size="16" /></span>
      <span class="grid size-8 shrink-0 place-items-center" aria-hidden="true"><Icon name="rotate-ccw" :size="16" /></span>
      <i class="mx-1 h-5 w-px shrink-0 bg-line-strong" aria-hidden="true" />
      <button
        v-for="[icon, label] in tools"
        :key="label"
        type="button"
        class="grid size-8 shrink-0 place-items-center rounded-card-sm transition hover:bg-accent-soft hover:text-accent"
        :class="activeTool === label && 'bg-accent-soft text-accent'"
        :aria-label="label"
        :aria-pressed="activeTool === label"
        @click="activeTool = label"
      >
        <Icon :name="icon" :size="16" />
      </button>
      <span class="grid size-8 shrink-0 place-items-center" aria-hidden="true"><Icon name="eye-off" :size="16" /></span>
      <i class="mx-1 h-5 w-px shrink-0 bg-line-strong" aria-hidden="true" />
      <span class="flex-1" />
      <button type="button" class="grid size-8 shrink-0 place-items-center rounded-card-sm transition hover:bg-accent-soft hover:text-accent" :class="inspector && 'text-accent'" aria-label="Show or hide the inspector" :aria-pressed="inspector" @click="inspector = !inspector">
        <Icon name="panel-right" :size="16" />
      </button>
    </div>

    <div
      class="grid grid-cols-[minmax(0,1fr)] transition-[grid-template-columns,grid-template-rows] duration-500 ease-out-soft md:grid-rows-1"
      :class="inspector ? 'grid-rows-[auto_1fr] md:grid-cols-[minmax(0,1fr)_250px]' : 'grid-rows-[auto_0fr] md:grid-cols-[minmax(0,1fr)_0px]'"
    >
      <!-- Canvas -->
      <div class="grid min-h-[280px] place-items-center bg-[repeating-conic-gradient(var(--bg-2)_0_25%,var(--bg)_0_50%)] bg-[length:18px_18px] p-4 sm:p-8">
        <!-- The sizer holds the height of the largest padding, so dragging Padding never moves the page. -->
        <div class="col-start-1 row-start-1 box-content aspect-[1/0.7165] w-full max-w-[560px] pt-[9.2%]" aria-hidden="true" />
        <div
          class="relative col-start-1 row-start-1 w-full max-w-[560px] transition-[padding] duration-700 ease-out-soft [perspective:1200px]"
          :style="{ padding: `${padding}%`, borderRadius: 'var(--radius-card)' }"
        >
          <i
            v-for="b in backgrounds"
            :key="b.id"
            class="absolute inset-0 rounded-[inherit] transition-opacity duration-700 ease-out-soft"
            :class="bg.id === b.id ? 'opacity-100' : 'opacity-0'"
            :style="{ background: b.css }"
          />
          <div
            class="relative overflow-hidden transition-all duration-700 ease-out-soft"
            :style="{
              borderRadius: `${radius}px`,
              boxShadow: shadow ? '0 30px 60px -20px rgba(3,30,40,.55), 0 8px 20px -8px rgba(3,30,40,.35)' : 'none',
              transform: tilt ? 'rotateX(14deg) rotateY(-16deg) rotateZ(3deg)' : 'none',
            }"
          >
            <ShotWindow photo="forest" redact radius="inherit">
              <svg class="absolute inset-0 size-full" viewBox="0 0 1600 900" aria-hidden="true">
                <defs>
                  <marker id="ah" viewBox="0 0 10 10" refX="6" refY="5" markerWidth="4" markerHeight="4" orient="auto-start-reverse">
                    <path d="M0 0 10 5 0 10z" fill="#ff4f6d" />
                  </marker>
                </defs>
                <path class="em-draw" pathLength="100" d="M380 720 C 520 760, 700 700, 820 560" fill="none" stroke="#ff4f6d" stroke-width="16" stroke-linecap="round" marker-end="url(#ah)" />
                <rect class="em-draw" pathLength="100" x="900" y="240" width="460" height="300" rx="40" fill="none" stroke="#ff4f6d" stroke-width="12" />
              </svg>
              <span class="absolute top-[72%] left-[8%] rounded-md bg-[#ff4f6d] px-2 py-0.5 text-[clamp(9px,1.6vw,14px)] font-semibold text-white shadow">Look here!</span>
              <span class="absolute top-[10%] left-[5%] grid size-[clamp(18px,3vw,28px)] place-items-center rounded-full bg-[#ff4f6d] text-[clamp(9px,1.4vw,13px)] font-bold text-white shadow">1</span>
            </ShotWindow>
          </div>
        </div>
      </div>

      <!-- Inspector -->
      <aside class="min-h-0 min-w-0 overflow-hidden transition-opacity duration-500 ease-out-soft" :class="!inspector && 'opacity-0'" :inert="!inspector" aria-label="Beautify inspector">
        <div class="grid h-full content-start gap-5 border-t border-line p-5 text-sm md:w-[250px] md:border-t-0 md:border-l">
          <div class="grid gap-2.5">
            <span class="text-xs font-semibold text-fg-3 uppercase">Background</span>
            <div class="grid grid-cols-6 gap-1.5 md:grid-cols-3">
              <button
                v-for="b in backgrounds"
                :key="b.id"
                type="button"
                class="aspect-[4/3] rounded-card-sm border-2 transition-all duration-300 hover:scale-105"
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
            <button type="button" class="rounded-card-sm border px-3 py-2 text-xs font-semibold transition" :class="shadow ? 'border-accent bg-accent-soft text-accent' : 'border-line text-fg-2'" :aria-pressed="shadow" @click="shadow = !shadow">Shadow</button>
            <button type="button" class="rounded-card-sm border px-3 py-2 text-xs font-semibold transition" :class="tilt ? 'border-accent bg-accent-soft text-accent' : 'border-line text-fg-2'" :aria-pressed="tilt" @click="tilt = !tilt">Camera 3D</button>
          </div>
        </div>
      </aside>
    </div>

    <!-- Footer: zoom and Copy -->
    <div class="flex h-10 items-center justify-between border-t border-line bg-surface-2 px-3 text-xs text-fg-2">
      <span class="font-mono">100%</span>
      <span class="flex items-center gap-1.5 rounded-card-sm px-2.5 py-1 font-medium"><Icon name="copy" :size="14" />Copy</span>
    </div>
  </div>
</template>

<style>
.em-draw {
  stroke-dasharray: 100;
  stroke-dashoffset: 100;
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
