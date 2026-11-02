interface PageSeo {
  title: string
  description: string
  path: string
  image?: string
}

// One place for title, description, canonical, Open Graph and Twitter tags.
export function usePageSeo(page: PageSeo) {
  const { siteUrl, siteName } = useRuntimeConfig().public
  const url = `${siteUrl}${page.path}`
  const image = `${siteUrl}${page.image ?? '/og.png'}`

  useHead({
    link: [{ rel: 'canonical', href: url }],
  })

  useSeoMeta({
    title: page.title,
    description: page.description,
    ogType: 'website',
    ogSiteName: siteName,
    ogTitle: page.title,
    ogDescription: page.description,
    ogUrl: url,
    ogImage: image,
    ogImageWidth: 1200,
    ogImageHeight: 630,
    ogImageAlt: 'easyshot — screenshots, annotation and screen recording for Linux Wayland',
    ogLocale: 'en_US',
    twitterCard: 'summary_large_image',
    twitterTitle: page.title,
    twitterDescription: page.description,
    twitterImage: image,
    robots: 'index, follow, max-image-preview:large',
  })

  return { url, image }
}
