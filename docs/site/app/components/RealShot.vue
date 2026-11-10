<script setup lang="ts">
const props = withDefaults(defineProps<{ shot: ShotKey; sizes?: string; eager?: boolean }>(), { sizes: '100vw', eager: false })
const s = computed(() => shots[props.shot])
</script>

<template>
  <img
    :src="shotUrl(shot)"
    :srcset="shotSrcset(shot)"
    :sizes="sizes"
    :alt="s.alt"
    :width="s.w"
    :height="s.h"
    :loading="eager ? 'eager' : 'lazy'"
    :fetchpriority="eager ? 'high' : undefined"
    decoding="async"
    class="block h-auto w-full rounded-card border border-line-strong bg-bg-2 shadow-float"
  />
</template>
