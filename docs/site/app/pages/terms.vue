<script setup lang="ts">
const { siteUrl } = useRuntimeConfig().public

usePageSeo({
  title: 'Terms of Service',
  description: 'The terms for easyshot Free, the free link sharing built into easyshot: how long shares last, the limits, what you may share and what is kept.',
  path: '/terms',
})

const sections = [
  { id: 'free', title: 'easyshot Free' },
  { id: 'duration', title: 'How long shares last' },
  { id: 'limits', title: 'Limits' },
  { id: 'content', title: 'What you may share' },
  { id: 'removal', title: 'Removal' },
  { id: 'own-bucket', title: 'Your own bucket' },
  { id: 'data', title: 'What we keep' },
  { id: 'warranty', title: 'No warranty' },
  { id: 'report', title: 'Reporting a share' },
  { id: 'changes', title: 'Changes to these terms' },
]

useHead({
  script: [
    {
      type: 'application/ld+json',
      innerHTML: JSON.stringify({
        '@context': 'https://schema.org',
        '@type': 'BreadcrumbList',
        itemListElement: [
          { '@type': 'ListItem', position: 1, name: 'Home', item: siteUrl },
          { '@type': 'ListItem', position: 2, name: 'Terms of Service', item: `${siteUrl}/terms` },
        ],
      }),
    },
  ],
})
</script>

<template>
  <div>
    <section class="relative overflow-hidden pt-[130px] pb-16 sm:pt-[150px]">
      <div class="container-x grid justify-items-center gap-6 text-center">
        <nav aria-label="Breadcrumb" class="rise text-[13px] text-fg-3">
          <NuxtLink to="/" class="hover-motion hover:text-accent">Home</NuxtLink> <span class="mx-1.5">/</span> <span class="text-fg-2">Terms</span>
        </nav>
        <h1 class="rise max-w-[820px] text-[clamp(38px,6.4vw,68px)] leading-[1.04]" style="--d: 0.08s">
          Terms of <span class="grad-text">Service</span>
        </h1>
        <p class="rise max-w-[600px] text-[clamp(17px,2vw,20px)] text-fg-2" style="--d: 0.16s">
          These terms cover easyshot Free, the link sharing built into easyshot. The app itself runs on your computer; these
          terms are about what you upload.
        </p>
        <p class="rise text-[13px] text-fg-3" style="--d: 0.24s">Last updated <time datetime="2026-11-11">11 November 2026</time></p>
      </div>
    </section>

    <div class="container-x grid gap-12 pb-28 lg:grid-cols-[220px_1fr] lg:gap-16">
      <aside class="hidden lg:block">
        <nav class="sticky top-28 grid gap-1" aria-label="On this page">
          <p class="mb-2 text-xs font-semibold tracking-wider text-fg-3 uppercase">On this page</p>
          <a v-for="s in sections" :key="s.id" :href="`#${s.id}`" class="rounded-lg px-2 py-1.5 text-[14px] text-fg-2 hover-motion hover:text-fg">{{ s.title }}</a>
        </nav>
      </aside>

      <article class="terms grid max-w-[760px] min-w-0 gap-14 text-[16px] leading-relaxed text-fg-2">
        <section id="free">
          <h2>easyshot Free</h2>
          <p>
            easyshot Free is the free sharing service built into easyshot. When you press Share and choose it, the app
            uploads the file and gives you a link. It costs nothing and needs no account.
          </p>
          <p>
            Links are unlisted, not private. A link is long and random and is not listed or searchable anywhere, but anyone
            who has it can open the page and download the file.
          </p>
        </section>

        <section id="duration">
          <h2>How long shares last</h2>
          <p>
            You pick 1, 3 or 7 days when you share. When that time is up, the link stops working and the file is deleted.
            Anyone who opens the link after that sees a plain “page not found”. You can delete a share earlier from the app.
          </p>
        </section>

        <section id="limits">
          <h2>Limits</h2>
          <ul>
            <li>Images up to 10 MB, and GIFs up to 20 MB.</li>
            <li>Videos up to 200 MB. easyshot re-encodes a bigger video to fit, which holds roughly 13 to 27 minutes of 1080p screen recording.</li>
            <li>A daily allowance for each install.</li>
          </ul>
          <p>These limits may change. Free sharing may also pause for a while when the service's storage is full.</p>
        </section>

        <section id="content">
          <h2>What you may share</h2>
          <p>Share only what you have the right to share. Do not share:</p>
          <ul>
            <li>anything illegal;</li>
            <li>malware;</li>
            <li>anything that harms, harasses or exploits anyone.</li>
          </ul>
          <p>Child sexual abuse material is reported to the authorities.</p>
        </section>

        <section id="removal">
          <h2>Removal</h2>
          <p>We may remove any share, at any time, and block an install that misuses the service.</p>
        </section>

        <section id="own-bucket">
          <h2>Your own bucket</h2>
          <p>
            If you choose your own bucket in Settings › Sharing, easyshot uploads straight from your computer to your
            storage. Your keys and your files never reach us. Your storage provider's terms apply, and your links last as
            long as you keep the files there.
          </p>
        </section>

        <section id="data">
          <h2>What we keep</h2>
          <p>For each share on easyshot Free, we keep:</p>
          <ul>
            <li>the file, until it expires;</li>
            <li>its title, its size, and when it was made and expires;</li>
            <li>a random ID made for your install;</li>
            <li>counters per IP address, hashed and replaced every day, used only to apply the limits.</li>
          </ul>
          <p>There is no account, and no tracking in the app. The service is hosted on Cloudflare.</p>
        </section>

        <section id="warranty">
          <h2>No warranty</h2>
          <p>
            easyshot Free is provided as is, without guarantees. We do not promise it will always be available, or that a
            share will stay up for its full time. Keep your own copy of anything that matters.
          </p>
        </section>

        <section id="report">
          <h2>Reporting a share</h2>
          <p>
            If you find a share that breaks these terms, email
            <a href="mailto:mah3uz@gmail.com" class="text-accent underline">mah3uz@gmail.com</a> with the link and what is
            wrong with it. We read every report and act on it.
          </p>
        </section>

        <section id="changes">
          <h2>Changes to these terms</h2>
          <p>
            We may change these terms. The date at the top says when they last changed. If you keep sharing after a change,
            the new terms apply.
          </p>
        </section>
      </article>
    </div>
  </div>
</template>

<style scoped>
.terms section {
  display: grid;
  gap: 16px;
  scroll-margin-top: 7rem;
}
.terms h2 {
  color: var(--fg);
  font-size: clamp(24px, 3vw, 32px);
}
.terms ul {
  display: grid;
  gap: 8px;
  padding-left: 1.25rem;
  list-style: disc;
}
.terms li::marker {
  color: var(--accent);
}
</style>
