<script setup lang="ts">
defineProps<{ n: number; id: string; title: string }>()
</script>

<template>
  <section :id="id" v-reveal class="grid scroll-mt-28 gap-5">
    <div class="flex items-center gap-4">
      <span class="grid size-11 shrink-0 place-items-center rounded-card font-display text-lg font-bold text-[#032830] shadow-card" style="background: var(--grad)">{{ n }}</span>
      <h2 class="text-[clamp(26px,3.4vw,36px)]">{{ title }}</h2>
    </div>
    <div class="prose-step grid gap-5 text-[16px] leading-relaxed text-fg-2">
      <slot />
    </div>
  </section>
</template>

<style scoped>
.prose-step :deep(p code),
.prose-step :deep(li code) {
  border-radius: 6px;
  background: var(--surface-2);
  border: 1px solid var(--line);
  padding: 1px 6px;
  color: var(--fg);
}
.prose-step :deep(strong) {
  color: var(--fg);
  font-weight: 600;
}
</style>
