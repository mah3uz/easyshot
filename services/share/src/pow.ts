export const NONCE = /^(0|[1-9][0-9]{0,19})$/;

export async function powDigest(challenge: string, nonce: string): Promise<Uint8Array> {
  return new Uint8Array(await crypto.subtle.digest("SHA-256", new TextEncoder().encode(`${challenge}:${nonce}`)));
}

export function leadingZeroBits(d: Uint8Array): number {
  let n = 0;
  for (const b of d) {
    if (b === 0) {
      n += 8;
      continue;
    }
    return n + Math.clz32(b) - 24;
  }
  return n;
}

export async function powOk(challenge: string, nonce: string, difficulty: number): Promise<boolean> {
  return NONCE.test(nonce) && leadingZeroBits(await powDigest(challenge, nonce)) >= difficulty;
}
