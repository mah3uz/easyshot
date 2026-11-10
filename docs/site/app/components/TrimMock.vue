<script setup lang="ts">
const frames = 9
const sliders = [
  { icon: 'volume-2', label: 'System sound', value: 50 },
  { icon: 'mic', label: 'Microphone', value: 100 },
]
const choices = ['MP4', '1080p', 'Balanced', 'Vivid']
</script>

<template>
  <div class="card grid gap-5 p-5 shadow-float sm:p-6" aria-hidden="true">
    <div class="relative">
      <div class="flex h-[64px] gap-[3px] overflow-hidden rounded-card-sm bg-teal-ink p-[3px] sm:h-[76px]">
        <span
          v-for="i in frames"
          :key="i"
          class="min-w-0 flex-1 rounded-[3px] bg-cover"
          :style="{ backgroundImage: `url(${photoUrl('desk', 400)}), url(/img/landscape.svg)`, backgroundPosition: `${(i - 1) * 12}% 50%` }"
        />
      </div>
      <div class="pointer-events-none absolute inset-y-0 left-0 w-[14%] rounded-l-card-sm bg-teal-ink/70" />
      <div class="pointer-events-none absolute inset-y-0 right-0 w-[22%] rounded-r-card-sm bg-teal-ink/70" />
      <div class="trim-range absolute inset-y-0 right-[22%] left-[14%] rounded-[6px] border-y-[3px] border-accent">
        <span class="trim-grip left-0 -translate-x-full rounded-l-[6px]"><i /></span>
        <span class="trim-grip right-0 translate-x-full rounded-r-[6px]"><i /></span>
        <span class="trim-head absolute -inset-y-2 w-0.5 rounded-full bg-white shadow-[0_0_0_1px_rgba(4,48,64,.35)]" />
      </div>
    </div>
    <div class="relative -mt-2 h-4 font-mono text-[11.5px] text-fg-2 tabular-nums">
      <span class="absolute left-[14%] -translate-x-1/2">00:03.2</span>
      <span class="absolute left-[78%] -translate-x-1/2">00:31.8</span>
    </div>

    <div class="grid gap-3">
      <div v-for="s in sliders" :key="s.label" class="grid grid-cols-[18px_96px_1fr_44px] items-center gap-2.5 text-[13px] text-fg-2 max-sm:grid-cols-[18px_1fr_40px]">
        <Icon :name="s.icon" :size="16" class="text-accent" />
        <span class="max-sm:hidden">{{ s.label }}</span>
        <span class="relative h-1.5 rounded-full bg-surface-2 ring-1 ring-line">
          <span class="absolute inset-y-0 left-0 rounded-full" :style="{ width: `${s.value / 2}%`, background: 'var(--grad)' }" />
          <span class="absolute top-1/2 size-4 -translate-x-1/2 -translate-y-1/2 rounded-full border-2 border-accent bg-surface shadow-soft" :style="{ left: `${s.value / 2}%` }" />
        </span>
        <span class="text-right font-mono text-[12px] tabular-nums">{{ s.value }}%</span>
      </div>
    </div>

    <div class="flex flex-wrap items-center gap-2 border-t border-line pt-4">
      <span v-for="c in choices" :key="c" class="rounded-full border border-line bg-surface-2 px-3 py-1 text-[12.5px] font-medium text-fg-2">{{ c }}</span>
      <span class="ml-auto inline-flex h-9 items-center gap-2 rounded-full px-4 text-[13px] font-semibold text-[#032830]" style="background: var(--grad)">
        <Icon name="download" :size="15" />Export
      </span>
    </div>
  </div>
</template>

<style scoped>
.trim-grip {
  position: absolute;
  top: -3px;
  bottom: -3px;
  width: 14px;
  display: grid;
  place-items: center;
  background: var(--accent);
}
.trim-grip i {
  width: 2px;
  height: 40%;
  border-radius: 2px;
  background: rgba(3, 40, 48, 0.55);
}
.trim-head {
  animation: trim-play 7s linear infinite;
}
@keyframes trim-play {
  from {
    left: 0%;
  }
  to {
    left: 100%;
  }
}
</style>
