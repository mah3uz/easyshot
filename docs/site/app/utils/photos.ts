// Unsplash photos for the feature tiles (https://unsplash.com/license), downloaded and served from public/img/photos
// as WebP at 400, 640, 1080, 1600 and 2200 px wide. `id` is the Unsplash photo ID they came from.
// Each sits on /img/landscape.svg, so a file that fails to load leaves a drawn landscape.
export const photos = {
  valley: { id: '1506744038136-46273834b3fb' },
  lake: { id: '1501785888041-af3ef285b470' },
  forest: { id: '1441974231531-c6227db76b6e' },
  fog: { id: '1470071459604-3b5ec3a7fe05' },
} as const

export type PhotoKey = keyof typeof photos

const widths = [400, 640, 1080, 1600, 2200] as const

export function photoUrl(key: PhotoKey, width = 1600) {
  const w = widths.find((x) => x >= width) ?? 2200
  return `/img/photos/${key}-${w}.webp`
}

export function photoSrcset(key: PhotoKey) {
  return widths.map((w) => `${photoUrl(key, w)} ${w}w`).join(', ')
}
