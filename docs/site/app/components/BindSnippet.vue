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
    file: '~/.config/easyshot/binds.lua',
    code: binds
      .map(([m, k, c]) => `hl.bind("${m.split(' ').join(' + ')} + ${k}", hl.dsp.exec_cmd("easyshot ${c}"))`)
      .join('\n'),
  },
  Hyprland: {
    file: '~/.config/easyshot/binds.conf',
    code: binds.map(([m, k, c]) => `bind = ${m}, ${k}, exec, easyshot ${c}`).join('\n'),
  },
  sway: {
    file: '~/.config/easyshot/binds.conf',
    code: binds.map(([, k, c]) => `bindsym Mod4+Shift+${k.toLowerCase()} exec easyshot ${c}`).join('\n'),
  },
  niri: {
    file: '~/.config/easyshot/binds.kdl',
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
    <TabPills v-model="active" :names="names" label="Compositor" class="w-fit" />
    <SwapPanel :swap-key="active">
      <CodeBlock :label="tabs[active].file" :code="tabs[active].code" role="tabpanel" />
    </SwapPanel>
  </div>
</template>
