<script setup lang="ts">
// The recording view: a highlighted area and the control bar, with a running clock.
const seconds = ref(12)
const paused = ref(false)
let timer: ReturnType<typeof setInterval> | undefined
onMounted(() => {
  timer = setInterval(() => {
    if (!paused.value) seconds.value = (seconds.value + 1) % 600
  }, 1000)
})
onBeforeUnmount(() => clearInterval(timer))
const clock = computed(() => `${String(Math.floor(seconds.value / 60)).padStart(2, '0')}:${String(seconds.value % 60).padStart(2, '0')}`)
</script>

<template>
  <div class="relative overflow-hidden rounded-[22px] border border-line-strong shadow-float">
    <div class="absolute inset-0 bg-[url(/img/wall-5.webp)] bg-cover bg-center" />
    <img :src="photoUrl('stars', 1600)" :srcset="photoSrcset('stars')" sizes="(min-width: 1024px) 640px, 100vw" alt="" class="absolute inset-0 size-full object-cover" width="1600" height="900" loading="lazy" onerror="this.style.display='none'" />
    <div class="relative aspect-[16/10]">
      <div class="absolute top-[6%] left-1/2 w-[50%] -translate-x-1/2 sm:w-[56%] rounded-xl shadow-[0_0_0_9999px_rgba(3,30,40,.5)]">
        <ShotWindow photo="desk" />
        <div class="rec-ring absolute -inset-[3px] rounded-[14px] border-2 border-[#ff4f6d]" />
      </div>

      <!-- Control bar -->
      <div class="absolute bottom-[4%] left-1/2 flex -translate-x-1/2 origin-bottom items-center max-sm:scale-[.82] gap-1 rounded-full border border-white/15 bg-teal-ink/85 p-1.5 text-white shadow-2xl backdrop-blur-xl">
        <button type="button" class="flex items-center gap-2 rounded-full bg-[#ff4f6d] px-3 py-1.5 text-xs font-semibold" aria-label="Stop recording">
          <span class="size-2.5 rounded-[3px] bg-white" />
          <span class="font-mono tabular-nums">{{ clock }}</span>
        </button>
        <button type="button" class="grid size-8 place-items-center rounded-full hover:bg-white/10" :aria-label="paused ? 'Resume' : 'Pause'" @click="paused = !paused">
          <Icon :name="paused ? 'circle-play' : 'circle-pause'" :size="17" />
        </button>
        <button type="button" class="grid size-8 place-items-center rounded-full hover:bg-white/10" aria-label="Restart" @click="seconds = 0">
          <Icon name="rotate-ccw" :size="16" />
        </button>
        <span class="grid size-8 place-items-center" aria-hidden="true"><Icon name="mic" :size="16" /></span>
        <button type="button" class="grid size-8 place-items-center rounded-full hover:bg-white/10" aria-label="Discard">
          <Icon name="trash" :size="16" />
        </button>
      </div>
    </div>
  </div>
</template>

<style scoped>
.rec-ring {
  animation: pulse 2s ease-in-out infinite;
}
@keyframes pulse {
  50% {
    opacity: 0.35;
  }
}
</style>
