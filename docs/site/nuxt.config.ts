import { fileURLToPath } from 'node:url'
import tailwindcss from '@tailwindcss/vite'

// Static site, generated with `nuxt generate` and served by Cloudflare Workers static assets.
const siteUrl = process.env.NUXT_PUBLIC_SITE_URL || 'https://easyshot.site'

export default defineNuxtConfig({
  compatibilityDate: '2025-10-01',
  devtools: { enabled: false },
  // Nuxt 4.6.4 resolves an unset host to `{}`, which crashes `nuxt dev`.
  devServer: { host: process.env.NUXT_HOST || 'localhost' },
  css: ['~/assets/css/main.css'],
  vite: { plugins: [tailwindcss()] },

  runtimeConfig: {
    public: {
      siteUrl,
      siteName: 'easyshot',
      repo: 'https://github.com/mah3uz/easyshot',
      plausible: {
        domain: new URL(siteUrl).hostname,
        endpoint: 'https://plus.testtlc.com/api/event',
      },
    },
  },

  app: {
    head: {
      htmlAttrs: { lang: 'en' },
      charset: 'utf-8',
      viewport: 'width=device-width, initial-scale=1, viewport-fit=cover',
      link: [
        { rel: 'icon', type: 'image/svg+xml', href: '/favicon.svg' },
        { rel: 'icon', type: 'image/png', sizes: '32x32', href: '/favicon-32.png' },
        { rel: 'apple-touch-icon', href: '/apple-touch-icon.png' },
        { rel: 'manifest', href: '/site.webmanifest' },
        { rel: 'preconnect', href: 'https://fonts.googleapis.com' },
        { rel: 'preconnect', href: 'https://fonts.gstatic.com', crossorigin: '' },
        {
          rel: 'stylesheet',
          href: 'https://fonts.googleapis.com/css2?family=Inter:wght@400;500;600&family=Inter+Tight:wght@600;700;800&family=JetBrains+Mono:wght@400;500&display=swap',
        },
      ],
      meta: [
        { name: 'theme-color', content: '#f6fafb', media: '(prefers-color-scheme: light)' },
        { name: 'theme-color', content: '#04212b', media: '(prefers-color-scheme: dark)' },
        { name: 'color-scheme', content: 'light dark' },
      ],
      script: [
        {
          // Applies the saved theme before first paint, so there is no flash.
          innerHTML:
            "(function(){var d=document.documentElement;d.classList.add('js');try{var t=localStorage.getItem('es-theme');if(t==='light'||t==='dark'){d.dataset.theme=t}}catch(e){}})()",
          tagPriority: 'critical',
        },
      ],
    },
    pageTransition: { name: 'page', mode: 'out-in' },
  },

  nitro: {
    // The one-line installer is served at /install.sh straight from its only copy.
    publicAssets: [{ dir: fileURLToPath(new URL('../../packaging/web', import.meta.url)) }],
    prerender: {
      crawlLinks: true,
      routes: ['/', '/how-to', '/sitemap.xml', '/robots.txt'],
    },
  },
})
