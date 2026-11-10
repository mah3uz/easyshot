<script setup lang="ts">
// The copy icon, crossfading to a check while `copied` is on.
defineProps<{ copied: boolean; size?: number }>()
</script>

<template>
  <span class="grid place-items-center" aria-hidden="true">
    <Icon name="copy" :size="size ?? 15" class="col-start-1 row-start-1 transition duration-300 ease-out-soft" :class="copied && 'scale-50 opacity-0'" />
    <Icon name="check" :size="size ?? 15" class="col-start-1 row-start-1 transition duration-300 ease-out-soft" :class="!copied && 'scale-50 opacity-0'" />
  </span>
</template>
