<script setup lang="ts">
const days = ref(7)
const pct = ref(42)
const done = computed(() => pct.value >= 100)
let timer: ReturnType<typeof setInterval> | undefined
let hold = 0

function share() {
  pct.value = 0
  hold = 0
}

onMounted(() => {
  if (matchMedia('(prefers-reduced-motion: reduce)').matches) return
  timer = setInterval(() => {
    if (!done.value) pct.value = Math.min(100, pct.value + 2)
    else if (++hold > 40) share()
  }, 80)
})
onBeforeUnmount(() => clearInterval(timer))
</script>

<template>
  <div class="relative overflow-hidden rounded-card border border-line-strong shadow-float">
    <div class="absolute inset-0 bg-[url(/img/wall-3.webp)] bg-cover bg-center" />
    <div class="relative grid justify-items-center gap-4 px-4 py-10 sm:px-10 sm:py-14">
      <div class="w-full max-w-[360px] rounded-card border border-line bg-surface p-4 text-fg shadow-2xl backdrop-blur-xl" role="group" aria-label="Share">
        <div class="flex items-center gap-3">
          <img :src="photoUrl('lake', 400)" alt="" width="64" height="40" loading="lazy" class="h-10 w-16 shrink-0 rounded-card-xs bg-[url(/img/landscape.svg)] bg-cover object-cover" />
          <span class="grid min-w-0 gap-0.5">
            <span class="text-[11.5px] font-medium text-fg-3">Title</span>
            <span class="truncate text-[14px] font-medium">Mountain lake</span>
          </span>
        </div>
        <p class="mt-4 text-[12px] font-medium text-fg-3">Link lasts</p>
        <div class="mt-1.5 grid grid-cols-3 gap-1 rounded-full bg-surface-2 p-1">
          <button
            v-for="d in [1, 3, 7]"
            :key="d"
            type="button"
            class="rounded-full py-1.5 text-[13px] font-medium hover-motion"
            :class="days === d ? 'bg-surface text-accent shadow-soft' : 'text-fg-2 hover:text-fg'"
            :aria-pressed="days === d"
            @click="days = d"
          >
            {{ d }} {{ d === 1 ? 'day' : 'days' }}
          </button>
        </div>
        <div class="mt-3 flex items-center gap-2 rounded-card-sm border border-line px-3 py-2 text-[13px]">
          <Icon name="cloud-upload" :size="15" class="shrink-0 text-accent" />
          <span class="flex-1">easyshot Free</span>
          <span class="text-[12px] text-fg-3">Unlisted link</span>
        </div>
        <button type="button" class="mt-4 grid h-9 w-full place-items-center rounded-full text-[13.5px] font-semibold text-[#032830] hover-motion hover:brightness-105" style="background: var(--grad)" @click="share">Share</button>
      </div>

      <div class="flex w-full max-w-[400px] items-center gap-3 rounded-card border border-line bg-surface p-2.5 pr-3.5 text-fg shadow-2xl backdrop-blur-xl" aria-hidden="true">
        <img :src="photoUrl('lake', 400)" alt="" width="56" height="40" loading="lazy" class="h-10 w-14 shrink-0 rounded-card-xs bg-[url(/img/landscape.svg)] bg-cover object-cover" />
        <span class="grid min-w-0 flex-1 gap-1.5">
          <b class="text-[13px] font-semibold text-balance">{{ done ? 'Shared' : `Link copied · Uploading ${pct} %` }}</b>
          <span v-if="done" class="truncate text-[12px] text-fg-3">Expires in {{ days }} {{ days === 1 ? 'day' : 'days' }}</span>
          <span v-else class="block h-1 overflow-hidden rounded-full bg-surface-2">
            <span class="block h-full origin-left rounded-full" style="background: var(--grad)" :style="{ transform: `scaleX(${pct / 100})` }" />
          </span>
        </span>
        <span v-if="done" class="flex shrink-0 gap-3 text-[12.5px] font-semibold text-accent"><span>Open</span><span>Copy Link</span></span>
        <span v-else class="shrink-0 text-[12.5px] font-semibold text-fg-2">Cancel</span>
      </div>
    </div>
  </div>
</template>
