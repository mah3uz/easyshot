export default defineEventHandler((event) => {
  const { siteUrl } = useRuntimeConfig().public
  const today = new Date().toISOString().slice(0, 10)
  const pages = [
    { path: '/', priority: '1.0' },
    { path: '/how-to', priority: '0.8' },
    { path: '/terms', priority: '0.3' },
  ]
  const urls = pages
    .map((p) => `  <url><loc>${siteUrl}${p.path}</loc><lastmod>${today}</lastmod><priority>${p.priority}</priority></url>`)
    .join('\n')
  setHeader(event, 'content-type', 'application/xml; charset=utf-8')
  return `<?xml version="1.0" encoding="UTF-8"?>\n<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">\n${urls}\n</urlset>\n`
})
