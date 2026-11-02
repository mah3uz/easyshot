// Unsplash photos, hotlinked from images.unsplash.com as the Unsplash licence allows.
// Each sits on /img/landscape.svg, so a photo that fails to load leaves a drawn landscape.
export const photos = {
  valley: { id: '1506744038136-46273834b3fb', alt: 'A mountain valley at sunrise', title: 'Valley at first light' },
  lake: { id: '1501785888041-af3ef285b470', alt: 'A lake below mountains at dusk', title: 'Lake at dusk' },
  forest: { id: '1441974231531-c6227db76b6e', alt: 'Sunlight through a green forest', title: 'Morning in the forest' },
  fog: { id: '1470071459604-3b5ec3a7fe05', alt: 'Fog over green hills', title: 'Fog over the hills' },
  stars: { id: '1519681393784-d120267933ba', alt: 'Snowy mountains under a starry sky', title: 'Night over the peaks' },
  desk: { id: '1498050108023-c5249f4df085', alt: 'A laptop showing code on a desk', title: 'Late-night build' },
} as const

export type PhotoKey = keyof typeof photos

export function photoUrl(key: PhotoKey, width = 1600) {
  return `https://images.unsplash.com/photo-${photos[key].id}?auto=format&fit=crop&w=${width}&q=75`
}

export function photoSrcset(key: PhotoKey) {
  return [640, 1080, 1600, 2200].map((w) => `${photoUrl(key, w)} ${w}w`).join(', ')
}
