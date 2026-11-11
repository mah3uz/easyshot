import { track } from '@plausible-analytics/tracker'

export function useTour() {
  const playing = useState('tour', () => false)

  function play() {
    if (playing.value) return
    playing.value = true
    track('Tour played', {})
  }

  return { playing, play }
}
