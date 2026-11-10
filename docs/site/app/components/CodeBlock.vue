<script setup lang="ts">
// A terminal-style code block with a copy button. `code` is the text that gets copied.
const props = defineProps<{ code: string; label?: string; prompt?: boolean }>()
const { copied, copy } = useCopy()
const lines = computed(() => props.code.split('\n'))
</script>

<template>
  <div class="code-block group">
    <div v-if="label" class="mb-2 text-[11px] font-semibold tracking-wider text-[#6f9aa3] uppercase">{{ label }}</div>
    <pre class="m-0 pr-10"><code><template v-for="(l, i) in lines" :key="i"><span v-if="prompt && l && !l.startsWith('#')" class="p">$ </span><span :class="l.startsWith('#') && 'c'">{{ l }}</span>{{ i < lines.length - 1 ? '\n' : '' }}</template></code></pre>
    <button
      type="button"
      class="absolute top-3 right-3 grid size-8 place-items-center rounded-card-xs text-[#9cc7c9] opacity-70 hover-motion hover:bg-white/10 hover:opacity-100 focus-visible:opacity-100"
      :aria-label="copied ? 'Copied' : 'Copy to clipboard'"
      @click="copy(code)"
    >
      <CopyIcon :copied="copied" />
    </button>
  </div>
</template>
