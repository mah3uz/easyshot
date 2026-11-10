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
const mic = ref(false)
const sound = ref(false)
const buttons = computed(() => [
  { id: 'mic', icon: mic.value ? 'mic-off' : 'mic', label: mic.value ? 'Unmute microphone' : 'Mute microphone', muted: mic.value, pressed: mic.value, act: () => (mic.value = !mic.value) },
  { id: 'sound', icon: sound.value ? 'volume-off' : 'volume-2', label: sound.value ? 'Unmute system sound' : 'Mute system sound', muted: sound.value, pressed: sound.value, act: () => (sound.value = !sound.value) },
  { id: 'pause', icon: paused.value ? 'circle-play' : 'circle-pause', label: paused.value ? 'Resume' : 'Pause', muted: false, pressed: undefined, act: () => (paused.value = !paused.value) },
  { id: 'restart', icon: 'rotate-ccw', label: 'Start over', muted: false, pressed: undefined, act: () => (seconds.value = 0) },
  { id: 'discard', icon: 'trash', label: 'Discard', muted: false, pressed: undefined, act: () => {} },
])
const clock = computed(() => `${String(Math.floor(seconds.value / 60)).padStart(2, '0')}:${String(seconds.value % 60).padStart(2, '0')}`)
</script>

<template>
  <div class="relative overflow-hidden rounded-card border border-line-strong shadow-float">
    <div class="absolute inset-0 bg-[url(/img/wall-5.webp)] bg-cover bg-center" />
    <img :src="photoUrl('stars', 1600)" :srcset="photoSrcset('stars')" sizes="(min-width: 1024px) 640px, 100vw" alt="" class="absolute inset-0 size-full object-cover" width="1600" height="900" loading="lazy" onerror="this.style.display='none'" />
    <div class="relative aspect-[16/10]">
      <div class="absolute top-[6%] left-1/2 w-[50%] -translate-x-1/2 sm:w-[56%] rounded-card-sm shadow-[0_0_0_9999px_rgba(3,30,40,.5)]">
        <ShotWindow photo="desk" />
        <div class="rec-ring absolute -inset-[3px] rounded-card border-2 border-[#ff4f6d]" />
      </div>

      <!-- Control bar, laid out as the app's: grip, Stop with the clock, a mute per sound, pause, restart, discard -->
      <div class="absolute bottom-[4%] left-1/2 flex h-[38px] -translate-x-1/2 origin-bottom overflow-hidden rounded-card border border-line bg-surface text-fg shadow-2xl backdrop-blur-xl max-sm:scale-[.78]" role="toolbar" aria-label="Recording controls">
        <span class="grid w-[18px] shrink-0 place-items-center pl-1 text-fg-3" aria-hidden="true"><Icon name="grip-vertical" :size="13" /></span>
        <button type="button" class="flex w-[132px] shrink-0 items-center gap-2 px-2.5 text-[13px] font-medium text-[#ff4f6d] hover:bg-accent-soft" aria-label="Stop recording">
          <span class="grid size-4 shrink-0 place-items-center rounded-full border-2 border-current"><span class="size-1.5 rounded-[1.5px] bg-current" /></span>
          <span class="flex-1 text-left">{{ paused ? 'Paused' : 'Stop' }}</span>
          <span class="font-mono tabular-nums">{{ clock }}</span>
        </button>
        <template v-for="b in buttons" :key="b.id">
          <i class="w-px shrink-0 bg-line" aria-hidden="true" />
          <button type="button" class="grid w-[42px] shrink-0 place-items-center hover:bg-accent-soft" :class="b.muted && 'text-[#ff4f6d]'" :aria-label="b.label" :aria-pressed="b.pressed" @click="b.act">
            <Icon :name="b.icon" :size="17" />
          </button>
        </template>
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
