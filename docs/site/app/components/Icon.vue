<script setup lang="ts">
// Lucide icons copied from the app's own set (assets/icons/lucide), inlined at build time.
const icons = import.meta.glob('~/assets/icons/*.svg', {
  query: '?raw',
  import: 'default',
  eager: true,
}) as Record<string, string>

const props = defineProps<{ name: string; size?: number }>()

const svg = computed(() => {
  const key = Object.keys(icons).find((k) => k.endsWith(`/${props.name}.svg`))
  const raw = key ? icons[key] ?? '' : ''
  const size = props.size ?? 20
  return raw
    .replace(/width="24"/, `width="${size}"`)
    .replace(/height="24"/, `height="${size}"`)
    .replace('<svg', '<svg aria-hidden="true" focusable="false"')
})
</script>

<template>
  <span class="icon" v-html="svg" />
</template>

<style scoped>
.icon {
  display: inline-flex;
  line-height: 0;
}
</style>
