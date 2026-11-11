(() => {
  const KEY = 'es-share-theme'
  const root = document.documentElement
  root.classList.add('js')

  let choice = 'system'
  try {
    const saved = localStorage.getItem(KEY)
    if (saved === 'light' || saved === 'dark') choice = saved
  } catch {}

  const apply = (next) => {
    choice = next
    root.dataset.choice = next
    if (next === 'system') delete root.dataset.theme
    else root.dataset.theme = next
  }
  apply(choice)

  const save = () => {
    try {
      if (choice === 'system') localStorage.removeItem(KEY)
      else localStorage.setItem(KEY, choice)
    } catch {}
  }

  const relative = new Intl.RelativeTimeFormat('en', { numeric: 'always' })
  const expiresText = (ms) => {
    if (ms <= 0) return 'Expired'
    const minutes = ms / 60000
    if (minutes < 60) return `Expires ${relative.format(Math.max(1, Math.round(minutes)), 'minute')}`
    const hours = minutes / 60
    if (hours < 24) return `Expires ${relative.format(Math.round(hours), 'hour')}`
    return `Expires ${relative.format(Math.round(hours / 24), 'day')}`
  }

  document.addEventListener('DOMContentLoaded', () => {
    const toggle = document.querySelector('[data-theme-toggle]')
    const label = () => {
      toggle.title = `Theme: ${choice}`
      toggle.setAttribute('aria-label', `Theme: ${choice}. Click to change.`)
    }
    label()
    toggle.addEventListener('click', (event) => {
      const next = { system: 'light', light: 'dark', dark: 'system' }[choice]
      const run = () => {
        apply(next)
        save()
        label()
      }
      if (!document.startViewTransition || matchMedia('(prefers-reduced-motion: reduce)').matches) return run()
      const x = event.clientX || innerWidth - 40
      const y = event.clientY || 40
      const r = Math.hypot(Math.max(x, innerWidth - x), Math.max(y, innerHeight - y))
      document.startViewTransition(run).ready.then(() => {
        root.animate(
          { clipPath: [`circle(0px at ${x}px ${y}px)`, `circle(${r}px at ${x}px ${y}px)`] },
          { duration: 650, easing: 'cubic-bezier(0.16, 1, 0.3, 1)', pseudoElement: '::view-transition-new(root)' },
        )
      })
    })

    // Wide media is held to the viewport height so the buttons stay in view; very tall media (a scrolled page) scrolls instead.
    for (const media of document.querySelectorAll('[data-fit]')) {
      const w = parseInt(media.getAttribute('width'), 10)
      const h = parseInt(media.getAttribute('height'), 10)
      const stage = media.closest('.stage')
      if (!(w > 0 && h > 0) || !stage) continue
      const byHeight = h > w * 2.2 ? '100%' : `calc(var(--cap) * ${(w / h).toFixed(4)})`
      stage.style.setProperty('--fw', `min(100%, ${w}px, ${byHeight})`)
      stage.dataset.fitted = ''
    }

    const title = document.querySelector('main h1')
    if (title && title.textContent.length > 60) title.classList.add('long')

    const expires = document.querySelector('[data-expires]')
    const at = expires ? Date.parse(expires.dataset.expires) : NaN
    if (!Number.isNaN(at)) {
      expires.textContent = expiresText(at - Date.now())
      expires.title = new Date(at).toLocaleString()
      expires.hidden = false
    }

    const copy = document.querySelector('[data-copy]')
    if (copy && navigator.clipboard) {
      const text = copy.querySelector('[data-copy-label]')
      const status = document.querySelector('[data-copy-status]')
      let timer
      copy.hidden = false
      copy.addEventListener('click', async () => {
        let ok = true
        try {
          await navigator.clipboard.writeText(copy.dataset.url)
        } catch {
          ok = false
        }
        copy.dataset.state = ok ? 'copied' : 'failed'
        text.textContent = ok ? 'Copied' : 'Could not copy'
        status.textContent = ok ? 'Link copied' : 'Could not copy the link'
        clearTimeout(timer)
        timer = setTimeout(() => {
          delete copy.dataset.state
          text.textContent = 'Copy link'
          status.textContent = ''
        }, 1600)
      })
    }

    if (document.querySelector('[data-reload]')) setTimeout(() => location.reload(), 5000)
  })
})()
