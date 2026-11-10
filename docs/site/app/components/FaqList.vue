<script setup lang="ts">
defineProps<{ items: { q: string; a: string }[] }>()
const open = ref<number | null>(0)
</script>

<template>
  <div class="divide-y divide-line overflow-hidden rounded-card border border-line bg-surface shadow-soft">
    <div v-for="(item, i) in items" :key="item.q">
      <h3 class="m-0 font-sans text-base font-semibold tracking-normal">
        <button
          :id="`faq-q-${i}`"
          type="button"
          class="flex w-full cursor-pointer items-center justify-between gap-6 px-6 py-5 text-left hover-motion hover:text-accent"
          :aria-expanded="open === i"
          :aria-controls="`faq-a-${i}`"
          @click="open = open === i ? null : i"
        >
          {{ item.q }}
          <span class="grid size-7 shrink-0 place-items-center rounded-full bg-surface-2 transition-[transform,background-color,color] duration-500 ease-out-soft" :class="open === i && 'rotate-45 bg-accent-soft text-accent'">
            <Icon name="plus" :size="15" />
          </span>
        </button>
      </h3>
      <div
        :id="`faq-a-${i}`"
        role="region"
        :aria-labelledby="`faq-q-${i}`"
        class="grid transition-[grid-template-rows] duration-500 ease-out-soft"
        :class="open === i ? 'grid-rows-[1fr]' : 'grid-rows-[0fr]'"
      >
        <div class="overflow-hidden" :inert="open !== i">
          <p class="px-6 pb-6 leading-relaxed text-fg-2" v-html="item.a" />
        </div>
      </div>
    </div>
  </div>
</template>
