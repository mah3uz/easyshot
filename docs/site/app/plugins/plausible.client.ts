import { init } from '@plausible-analytics/tracker'

export default defineNuxtPlugin(() => {
  const { domain, endpoint } = useRuntimeConfig().public.plausible
  init({ domain, endpoint, outboundLinks: true })
})
