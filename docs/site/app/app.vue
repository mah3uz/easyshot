<script setup lang="ts">
const { siteUrl } = useRuntimeConfig().public
useHead({
  titleTemplate: (t) => (t && !t.startsWith('easyshot') ? `${t} · easyshot` : t || 'easyshot'),
  script: [
    {
      type: 'application/ld+json',
      innerHTML: JSON.stringify({
        '@context': 'https://schema.org',
        '@type': 'Organization',
        name: 'easyshot',
        url: siteUrl,
        logo: `${siteUrl}/logo-512.png`,
        sameAs: ['https://github.com/mah3uz/easyshot'],
      }),
    },
  ],
})
</script>

<template>
  <div class="flex min-h-dvh flex-col">
    <div class="page-bg" aria-hidden="true"><i class="b1" /><i class="b2" /><i class="b3" /></div>
    <a href="#main" class="sr-only focus:not-sr-only focus:fixed focus:top-3 focus:left-3 focus:z-[60] focus:rounded-full focus:bg-accent focus:px-4 focus:py-2 focus:text-[#032830]">Skip to content</a>
    <SiteHeader />
    <main id="main" class="flex-1">
      <NuxtPage />
    </main>
    <SiteFooter />
  </div>
</template>
