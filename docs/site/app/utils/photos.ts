// Unsplash photos (https://unsplash.com/license), downloaded and served from public/img/photos
// as WebP at 400, 640, 1080, 1600 and 2200 px wide. `id` is the Unsplash photo ID they came from.
// Each sits on /img/landscape.svg, so a file that fails to load leaves a drawn landscape.
export const photos = {
  valley: { id: '1506744038136-46273834b3fb', alt: 'A misty valley with a river and pine trees at dawn', title: 'Valley at first light' },
  lake: { id: '1501785888041-af3ef285b470', alt: 'A rowing boat on a turquoise mountain lake', title: 'Mountain lake' },
  forest: { id: '1441974231531-c6227db76b6e', alt: 'Sunlight through tall trees in a forest', title: 'Morning in the forest' },
  fog: { id: '1470071459604-3b5ec3a7fe05', alt: 'Fog rolling over green hills at sunrise', title: 'Fog over the hills' },
  stars: { id: '1519681393784-d120267933ba', alt: 'The Milky Way over snowy mountains', title: 'Night over the peaks' },
  desk: { id: '1498050108023-c5249f4df085', alt: 'A laptop showing code on a white desk', title: 'Late-night build' },
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
