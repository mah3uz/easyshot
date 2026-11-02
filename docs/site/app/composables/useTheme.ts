export type ThemeChoice = 'system' | 'light' | 'dark'

const KEY = 'es-theme'

export function useTheme() {
  const choice = useState<ThemeChoice>('theme-choice', () => 'system')

  const apply = (value: ThemeChoice) => {
    choice.value = value
    if (!import.meta.client) return
    const root = document.documentElement
    root.classList.add('theme-switching')
    if (value === 'system') delete root.dataset.theme
    else root.dataset.theme = value
    try {
      if (value === 'system') localStorage.removeItem(KEY)
      else localStorage.setItem(KEY, value)
    } catch {
      // Storage can be blocked; the choice still applies to this page.
    }
    window.setTimeout(() => root.classList.remove('theme-switching'), 450)
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

  const cycle = () => {
    const order: ThemeChoice[] = ['system', 'light', 'dark']
    apply(order[(order.indexOf(choice.value) + 1) % order.length] ?? 'system')
  }

  return { choice, apply, load, cycle }
}
