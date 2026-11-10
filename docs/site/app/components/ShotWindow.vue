<script setup lang="ts">
// A captured web page: a photo-sharing page in a browser window, drawn in HTML.
// It scales with its container (container query units), so it works from deck-card size
// to full width. `redact` covers the e-mail with a pixelate shape, as Smart Redaction does.
const props = withDefaults(defineProps<{ photo: PhotoKey; redact?: boolean; eager?: boolean; chrome?: boolean }>(), {
  redact: false,
  eager: false,
  chrome: true,
})
const p = computed(() => photos[props.photo])
</script>

<template>
  <div class="@container overflow-hidden bg-white text-[#0a2631]" style="border-radius: 0.8cqw">
    <div v-if="chrome" class="flex items-center gap-[1.4cqw] bg-[#eef3f5] px-[2cqw]" style="height: 6.4cqw">
      <span class="flex gap-[0.9cqw]"><i v-for="c in ['#ff5f57', '#febc2e', '#28c840']" :key="c" class="block rounded-full" :style="{ background: c, width: '1.5cqw', height: '1.5cqw' }" /></span>
      <span class="mx-auto truncate rounded-full bg-white px-[2cqw] text-[#5c7680]" style="font-size: 1.8cqw; line-height: 3.6cqw; width: 46%">unsplash.com/photos/{{ photo }}</span>
    </div>
    <div class="relative aspect-[16/9] bg-[url(/img/landscape.svg)] bg-cover bg-center">
      <img
        :src="photoUrl(photo, 1080)"
        :srcset="photoSrcset(photo)"
        sizes="(min-width: 1024px) 700px, 90vw"
        :alt="p.alt"
        class="absolute inset-0 size-full object-cover"
        width="1600"
        height="900"
        :loading="eager ? 'eager' : 'lazy'"
        decoding="async"
       
        onerror="this.style.display='none'"
      />
      <slot />
    </div>
    <div class="flex items-center gap-[1.6cqw] px-[2.4cqw]" style="height: 9cqw">
      <span class="shrink-0 rounded-full bg-[linear-gradient(135deg,#62edd4,#1388a6)]" style="width: 4.6cqw; height: 4.6cqw" />
      <span class="grid min-w-0 leading-tight">
        <b class="truncate font-semibold" style="font-size: 2.1cqw">{{ p.title }}</b>
        <span class="relative w-fit text-[#6b8590]" style="font-size: 1.7cqw">
          jane.cooper@example.com
          <span v-if="redact" class="shot-redact absolute -inset-x-[0.6cqw] -inset-y-[0.3cqw] rounded-[0.6cqw]" />
        </span>
      </span>
      <span class="ml-auto shrink-0 rounded-[1cqw] bg-[#0a2631] px-[2cqw] font-semibold text-white" style="font-size: 1.8cqw; line-height: 4.4cqw">Download</span>
    </div>
  </div>
</template>

<style>
.shot-redact {
  background:
    repeating-linear-gradient(90deg, rgba(255, 255, 255, 0.22) 0 0.9cqw, transparent 0.9cqw 1.8cqw),
    repeating-linear-gradient(0deg, rgba(4, 48, 64, 0.16) 0 0.9cqw, transparent 0.9cqw 1.8cqw), #8fb1ba;
  transform-origin: left;
  transform: scaleX(0);
  transition: transform 0.9s cubic-bezier(0.65, 0, 0.35, 1) 0.6s;
}
.is-in .shot-redact {
  transform: scaleX(1);
}
</style>
