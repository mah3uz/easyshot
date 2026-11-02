// v-reveal: fades an element up the first time it scrolls into view.
// On the server it only marks the element, so the content is in the HTML for crawlers.
export default defineNuxtPlugin((nuxtApp) => {
  let observer: IntersectionObserver | null = null

  const getObserver = () => {
    if (observer || typeof IntersectionObserver === 'undefined') return observer
    observer = new IntersectionObserver(
      (entries) => {
        for (const e of entries) {
          if (e.isIntersecting) {
            e.target.classList.add('is-in')
            observer?.unobserve(e.target)
          }
        }
      },
      { rootMargin: '0px 0px -8% 0px', threshold: 0.12 },
    )
    return observer
  }

  nuxtApp.vueApp.directive<HTMLElement, number | undefined>('reveal', {
    getSSRProps(binding) {
      return {
        'data-reveal': '',
        style: binding.value ? `--d:${binding.value}s` : undefined,
      }
    },
    mounted(el, binding) {
      el.setAttribute('data-reveal', '')
      if (binding.value) el.style.setProperty('--d', `${binding.value}s`)
      const io = getObserver()
      if (io) io.observe(el)
      else el.classList.add('is-in')
    },
    unmounted(el) {
      observer?.unobserve(el)
    },
  })
})
