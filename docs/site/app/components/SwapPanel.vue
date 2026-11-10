<script setup lang="ts">
// Crossfades its content when `swapKey` changes and eases its height to the new content's,
// so nothing below it jumps.
defineProps<{ swapKey: string }>()

const inner = ref<HTMLElement>()
const height = ref<number>()
const instant = ref(true)
let width = 0
let ro: ResizeObserver | undefined

onMounted(() => {
  ro = new ResizeObserver(() => {
    const el = inner.value
    if (!el) return
    // A resize of the page reflows the text; follow it at once rather than easing.
    instant.value = el.offsetWidth !== width
    width = el.offsetWidth
    height.value = el.offsetHeight
  })
  if (inner.value) ro.observe(inner.value)
})
onBeforeUnmount(() => ro?.disconnect())
</script>

<template>
  <div
    class="overflow-hidden"
    :class="!instant && 'transition-[height] duration-500 ease-out-soft'"
    :style="height === undefined ? undefined : { height: `${height}px` }"
  >
    <div ref="inner" class="relative">
      <Transition
        enter-active-class="transition duration-300 ease-out-soft"
        enter-from-class="opacity-0 translate-y-1"
        leave-active-class="transition duration-200 ease-out-soft absolute inset-x-0 top-0"
        leave-to-class="opacity-0"
      >
        <div :key="swapKey"><slot /></div>
      </Transition>
    </div>
  </div>
</template>
