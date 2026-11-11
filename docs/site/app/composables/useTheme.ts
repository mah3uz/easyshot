export type ThemeChoice = 'system' | 'light' | 'dark'

const KEY = 'es-theme'

export function useTheme() {
  const choice = useState<ThemeChoice>('theme-choice', () => 'system')

  const apply = (value: ThemeChoice) => {
    choice.value = value
    if (!import.meta.client) return
    const root = document.documentElement
    if (value === 'system') delete root.dataset.theme
    else root.dataset.theme = value
    try {
      if (value === 'system') localStorage.removeItem(KEY)
      else localStorage.setItem(KEY, value)
    } catch {
      // Storage can be blocked; the choice still applies to this page.
    }
  }

  const load = () => {
    if (!import.meta.client) return
    let saved: string | null = null
    try {
      saved = localStorage.getItem(KEY)
    } catch {
      saved = null
    }
    choice.value = saved === 'light' || saved === 'dark' ? saved : 'system'
  }

  const cycle = (event: MouseEvent) => {
    const order: ThemeChoice[] = ['system', 'light', 'dark']
    const next = order[(order.indexOf(choice.value) + 1) % order.length] ?? 'system'
    if (!('startViewTransition' in document) || matchMedia('(prefers-reduced-motion: reduce)').matches) return apply(next)

    const x = event.clientX || innerWidth - 40
    const y = event.clientY || 40
    const radius = Math.hypot(Math.max(x, innerWidth - x), Math.max(y, innerHeight - y))
    // Fading every element's colour at once stuttered under the backdrop blurs; one clipped snapshot does not.
    document
      .startViewTransition(async () => {
        apply(next)
        await nextTick()
      })
      .ready.then(() => {
        document.documentElement.animate(
          { clipPath: [`circle(0px at ${x}px ${y}px)`, `circle(${radius}px at ${x}px ${y}px)`] },
          { duration: 650, easing: 'cubic-bezier(0.16, 1, 0.3, 1)', pseudoElement: '::view-transition-new(root)' },
        )
      })
  }

  return { choice, apply, load, cycle }
}
