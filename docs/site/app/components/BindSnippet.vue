<script setup lang="ts">
// Key bindings for each compositor, in the formats easyshot writes to its binds file.
const binds = [
  ['SUPER SHIFT', '4', 'capture region'],
  ['SUPER SHIFT', '3', 'capture fullscreen'],
  ['SUPER SHIFT', '1', 'capture previous'],
  ['SUPER SHIFT', '2', 'capture-bar'],
  ['SUPER SHIFT', '5', 'record'],
  ['SUPER SHIFT', 'O', 'ocr'],
  ['SUPER SHIFT', 'C', 'color-picker'],
] as const

const tabs = {
  'Hyprland (Lua)': {
    file: '~/.config/hypr/hyprland.lua',
    code: binds
      .map(([m, k, c]) => `hl.bind("${m.split(' ').join(' + ')} + ${k}", hl.dsp.exec_cmd("easyshot ${c}"))`)
      .join('\n'),
  },
  Hyprland: {
    file: '~/.config/hypr/hyprland.conf',
    code: binds.map(([m, k, c]) => `bind = ${m}, ${k}, exec, easyshot ${c}`).join('\n'),
  },
  sway: {
    file: '~/.config/sway/config',
    code: binds.map(([, k, c]) => `bindsym Mod4+Shift+${k.toLowerCase()} exec easyshot ${c}`).join('\n'),
  },
  niri: {
    file: '~/.config/niri/config.kdl — inside binds { }',
    code: binds
      .map(([, k, c]) => `Mod+Shift+${k} { spawn ${['easyshot', ...c.split(' ')].map((w) => `"${w}"`).join(' ')}; }`)
      .join('\n'),
  },
} as const

type Tab = keyof typeof tabs
const active = ref<Tab>('Hyprland (Lua)')
const names = Object.keys(tabs) as Tab[]
</script>

<template>
  <div class="grid gap-3">
    <div class="inline-flex w-fit max-w-full flex-wrap gap-1 rounded-2xl sm:rounded-full border border-line bg-surface p-1 shadow-soft" role="tablist" aria-label="Compositor">
      <button
        v-for="n in names"
        :key="n"
        type="button"
        role="tab"
        :aria-selected="active === n"
        class="relative rounded-full px-4 py-1.5 text-sm font-medium transition-colors duration-300"
        :class="active === n ? 'bg-accent text-[#032830]' : 'text-fg-2 hover:text-fg'"
        @click="active = n"
      >
        {{ n }}
      </button>
    </div>
    <Transition mode="out-in" enter-active-class="transition duration-300 ease-out-soft" enter-from-class="opacity-0 translate-y-1" leave-active-class="transition duration-150" leave-to-class="opacity-0">
      <CodeBlock :key="active" :label="tabs[active].file" :code="tabs[active].code" role="tabpanel" />
    </Transition>
  </div>
</template>
