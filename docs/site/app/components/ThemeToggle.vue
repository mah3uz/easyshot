<script setup lang="ts">
const { choice, cycle, load } = useTheme()
onMounted(load)

const label = computed(
  () =>
    ({ system: 'Theme: system', light: 'Theme: light', dark: 'Theme: dark' })[choice.value],
)
</script>

<template>
  <button
    type="button"
    class="relative grid size-[38px] cursor-pointer place-items-center overflow-hidden rounded-full border border-line bg-surface text-fg-2 shadow-soft transition hover:border-accent hover:text-fg"
    :aria-label="`${label}. Click to change.`"
    :title="label"
    @click="cycle"
  >
    <Transition
      enter-active-class="transition duration-500 ease-out-soft"
      enter-from-class="opacity-0 rotate-[-90deg] scale-50"
      leave-active-class="transition duration-300 ease-out-soft absolute"
      leave-to-class="opacity-0 rotate-90 scale-50"
    >
      <svg v-if="choice === 'light'" key="l" width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" aria-hidden="true">
        <circle cx="12" cy="12" r="4" />
        <path d="M12 2v2M12 20v2M4.93 4.93l1.41 1.41M17.66 17.66l1.41 1.41M2 12h2M20 12h2M6.34 17.66l-1.41 1.41M19.07 4.93l-1.41 1.41" />
      </svg>
      <svg v-else-if="choice === 'dark'" key="d" width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
        <path d="M12 3a6 6 0 0 0 9 9 9 9 0 1 1-9-9Z" />
      </svg>
      <svg v-else key="s" width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
        <rect x="2" y="3" width="20" height="14" rx="2" />
        <path d="M8 21h8M12 17v4" />
      </svg>
    </Transition>
  </button>
</template>
