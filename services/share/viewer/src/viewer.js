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

    // Media is contained in the space the page leaves it; an image too tall to stay readable that way fills the frame from its top instead.
    for (const media of document.querySelectorAll('[data-fit]')) {
      const w = parseInt(media.getAttribute('width'), 10)
      const h = parseInt(media.getAttribute('height'), 10)
      const stage = media.closest('.stage')
      if (!(w > 0 && h > 0) || !stage) continue
      stage.style.setProperty('--fw', `min(100%, ${w}px, (100cqh - 2px) * ${(w / h).toFixed(4)} + 2px)`)
      stage.style.setProperty('--nw', `${w}px`)
      stage.style.setProperty('--nh', `${h + 2}px`)
      stage.style.setProperty('--ar', (h / w).toFixed(4))
      stage.dataset.fitted = ''
      if (media.tagName !== 'IMG' || !window.ResizeObserver) continue
      new ResizeObserver(([entry]) => {
        const { width, height } = entry.contentRect
        const shown = Math.min(w, width, (height * w) / h)
        stage.toggleAttribute('data-tall', shown < 0.45 * Math.min(width, w))
      }).observe(stage)
    }

    const full = document.querySelector('[data-full]')
    const opener = document.querySelector('[data-full-open]')
    const source = document.querySelector('img.media')
    if (full && opener && source && full.showModal) {
      const frame = source.closest('.window')
      const big = full.querySelector('img')
      const fade = [full.querySelector('.full-bg'), full.querySelector('.full-close')]
      const w = parseInt(source.getAttribute('width'), 10)
      const h = parseInt(source.getAttribute('height'), 10)
      const ease = getComputedStyle(root).getPropertyValue('--ease-out-soft').trim() || 'ease-out'
      const still = () => matchMedia('(prefers-reduced-motion: reduce)').matches
      let moving = []
      let closing = false

      // Contain, but a scroll capture contained to the viewport would be a sliver: it fills the width and scrolls instead.
      const fit = () => {
        const shown = Math.min(w, innerWidth, (innerHeight * w) / h)
        full.toggleAttribute('data-tall', shown < 0.45 * Math.min(w, innerWidth))
      }

      const move = (opening, duration) => {
        const a = frame.getBoundingClientRect()
        const b = big.getBoundingClientRect()
        const scale = a.width / b.width
        const r = getComputedStyle(big).borderTopLeftRadius
        // The page may show only the top of a tall image: the rest is clipped away at the start.
        const cut = Math.max(0, b.height - a.height / scale)
        const order = (k) => (opening ? k : [...k].reverse())
        const timing = { duration, easing: ease, fill: 'forwards' }
        moving = [
          big.animate(
            {
              transform: order([`translate(${a.left - b.left}px, ${a.top - b.top}px) scale(${scale})`, 'none']),
              clipPath: order([`inset(0 0 ${cut}px 0 round ${r})`, `inset(0 0 0 0 round ${r})`]),
            },
            timing,
          ),
          ...fade.map((el) => el.animate({ opacity: order([0, 1]) }, timing)),
        ]
        return Promise.all(moving.map((m) => m.finished)).catch(() => {})
      }

      const hide = async () => {
        if (closing || !full.open) return
        closing = true
        moving.forEach((m) => m.cancel())
        if (!still()) await move(false, 320)
        full.close()
      }

      opener.addEventListener('click', async () => {
        if (!big.getAttribute('src')) {
          if (w > 0 && h > 0) {
            big.width = w
            big.height = h
          }
          big.alt = source.alt
          big.src = source.currentSrc || source.src
          await big.decode().catch(() => {})
        }
        closing = false
        if (innerWidth > root.clientWidth) root.style.scrollbarGutter = 'stable'
        fit()
        full.showModal()
        if (!still()) move(true, 460)
      })
      full.addEventListener('cancel', (event) => {
        event.preventDefault()
        hide()
      })
      full.addEventListener('click', (event) => {
        if (!event.target.closest('img')) hide()
      })
      full.addEventListener('close', () => {
        moving.forEach((m) => m.cancel())
        moving = []
        closing = false
        root.style.scrollbarGutter = ''
        opener.focus({ preventScroll: true })
      })
      source.addEventListener('click', () => opener.click())
      addEventListener('resize', () => full.open && fit())
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
