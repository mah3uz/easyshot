// Real easyshot screenshots in public/img/shots, cropped from 2560×1440 captures at 2× scale,
// served as WebP at the photo widths up to each crop's own width (`<name>-<width>.webp`).
export const shots = {
  hero: { w: 2200, h: 1098, alt: 'easyshot selecting a window on a web page, the rest dimmed, with the size badge and a card in the deck' },
  deck: { w: 200, h: 146, alt: 'A deck card under the pointer, showing Copy, Save, pin, preview, close, edit and delete' },
  editor: { w: 2560, h: 1236, alt: 'The easyshot editor with a box, an arrow, text and a numbered step on a violet background' },
  record: { w: 1600, h: 840, alt: 'An area recording in progress, the rest of the screen dimmed' },
  recbar: { w: 400, h: 80, alt: 'The recording bar: Stop with the clock, mute sound, mute microphone, pause, start over and discard' },
} as const

export type ShotKey = keyof typeof shots

const widths = [400, 640, 1080, 1600, 2200]

function shotWidths(key: ShotKey) {
  const w = shots[key].w
  const fit = widths.filter((x) => x <= w)
  return fit.length ? fit : [w]
}

export function shotUrl(key: ShotKey, width = 1600) {
  const ws = shotWidths(key)
  return `/img/shots/${key}-${ws.find((x) => x >= width) ?? ws[ws.length - 1]}.webp`
}

export function shotSrcset(key: ShotKey) {
  return shotWidths(key).map((w) => `/img/shots/${key}-${w}.webp ${w}w`).join(', ')
}
