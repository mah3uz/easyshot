<script setup lang="ts" generic="T extends string">
// A row of tabs with one accent pill that slides to the chosen tab.
const props = defineProps<{ names: readonly T[]; label: string }>()
const active = defineModel<T>({ required: true })

const buttons = ref<HTMLElement[]>([])
const pill = ref({ x: 0, y: 0, w: 0, h: 0 })
const measured = ref(false)
const animate = ref(false)

function measure() {
  const el = buttons.value[props.names.indexOf(active.value)]
  if (!el) return
  pill.value = { x: el.offsetLeft, y: el.offsetTop, w: el.offsetWidth, h: el.offsetHeight }
  measured.value = true
}

onMounted(() => {
  measure()
  requestAnimationFrame(() => requestAnimationFrame(() => (animate.value = true)))
  window.addEventListener('resize', measure)
})
onBeforeUnmount(() => window.removeEventListener('resize', measure))
watch(active, () => nextTick(measure))
</script>

<template>
  <div class="relative flex max-w-full flex-wrap gap-1 rounded-card border border-line bg-surface p-1 shadow-soft sm:rounded-full" role="tablist" :aria-label="label">
    <span
      v-if="measured"
      class="pointer-events-none absolute top-0 left-0 rounded-full bg-accent"
      :class="animate ? 'transition-[transform,width,height] duration-300 ease-out-soft' : ''"
      :style="{ width: pill.w + 'px', height: pill.h + 'px', transform: `translate(${pill.x}px, ${pill.y}px)` }"
      aria-hidden="true"
    />
    <button
      v-for="(n, i) in names"
      :key="n"
      :ref="(el) => { if (el) buttons[i] = el as HTMLElement }"
      type="button"
      role="tab"
      :aria-selected="active === n"
      class="relative rounded-full px-4 py-1.5 text-sm font-medium hover-motion"
      :class="[active === n ? 'text-[#032830]' : 'text-fg-2 hover:text-fg', active === n && !measured ? 'bg-accent' : '']"
      @click="active = n"
    >
      {{ n }}
    </button>
  </div>
</template>
