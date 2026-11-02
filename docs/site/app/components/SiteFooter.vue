<script setup lang="ts">
const { repo } = useRuntimeConfig().public
const year = new Date().getFullYear()
const cols = [
  {
    title: 'Product',
    links: [
      { label: 'Features', to: '/#features' },
      { label: 'Annotate', to: '/#annotate' },
      { label: 'Record', to: '/#record' },
      { label: 'FAQ', to: '/#faq' },
    ],
  },
  {
    title: 'Learn',
    links: [
      { label: 'How-to guide', to: '/how-to' },
      { label: 'Keyboard shortcuts', to: '/how-to#shortcuts' },
      { label: 'Troubleshooting', to: '/how-to#troubleshooting' },
    ],
  },
  {
    title: 'Project',
    links: [
      { label: 'Source code', href: repo },
      { label: 'Manual', href: `${repo}/blob/main/docs/MANUAL.md` },
      { label: 'Changelog', href: `${repo}/blob/main/CHANGELOG.md` },
      { label: 'Report an issue', href: `${repo}/issues` },
    ],
  },
]
</script>

<template>
  <footer class="border-t border-line bg-bg-2/60">
    <div class="container-x grid gap-12 py-16 md:grid-cols-[1.4fr_repeat(3,1fr)]">
      <div class="grid content-start gap-4">
        <NuxtLink to="/" class="flex items-center gap-2.5">
          <img src="/logo.svg" alt="" width="30" height="30" class="size-[30px]" />
          <span class="font-display text-lg font-bold">easyshot</span>
        </NuxtLink>
        <p class="max-w-[300px] text-[14.5px] leading-relaxed text-fg-2">
          Free and open-source screenshots, annotation and screen recording for Linux on Wayland.
        </p>
        <p class="text-[13px] text-fg-3">Made with Rust and GPUI</p>
      </div>
      <nav v-for="c in cols" :key="c.title" :aria-label="c.title" class="grid content-start gap-3">
        <h2 class="font-sans text-[13px] font-semibold tracking-wide text-fg-3 uppercase">{{ c.title }}</h2>
        <template v-for="l in c.links" :key="l.label">
          <NuxtLink v-if="l.to" :to="l.to" class="w-fit text-[14.5px] text-fg-2 transition-colors hover:text-accent">{{ l.label }}</NuxtLink>
          <a v-else :href="l.href" rel="noopener" class="w-fit text-[14.5px] text-fg-2 transition-colors hover:text-accent">{{ l.label }}</a>
        </template>
      </nav>
    </div>
    <div class="container-x flex flex-wrap items-center justify-between gap-3 border-t border-line py-6 text-[13px] text-fg-3">
      <span>© {{ year }} easyshot contributors</span>
    </div>
  </footer>
</template>
