const enc = new TextEncoder();
const MAC_LEN = 32;

export function b64url(bytes: Uint8Array): string {
  let s = "";
  for (const b of bytes) s += String.fromCharCode(b);
  return btoa(s).replace(/\+/g, "-").replace(/\//g, "_").replace(/=+$/, "");
}

export function fromB64url(s: string): Uint8Array | null {
  if (!/^[A-Za-z0-9_-]*$/.test(s) || s.length % 4 === 1) return null;
  const bin = atob(s.replace(/-/g, "+").replace(/_/g, "/"));
  const out = new Uint8Array(bin.length);
  for (let i = 0; i < bin.length; i++) out[i] = bin.charCodeAt(i);
  return out;
}

export function random(n: number): Uint8Array {
  return crypto.getRandomValues(new Uint8Array(n));
}

async function hmacKey(secret: string): Promise<CryptoKey> {
  // An unset or short secret would make every token forgeable, so fail closed.
  if (typeof secret !== "string" || secret.length < 32) throw new Error("a signing secret is missing or shorter than 32 characters");
  return crypto.subtle.importKey("raw", enc.encode(secret), { name: "HMAC", hash: "SHA-256" }, false, ["sign", "verify"]);
}

async function sign(secret: string, data: Uint8Array): Promise<Uint8Array> {
  return new Uint8Array(await crypto.subtle.sign("HMAC", await hmacKey(secret), data));
}

async function verify(secret: string, mac: Uint8Array, data: Uint8Array): Promise<boolean> {
  return crypto.subtle.verify("HMAC", await hmacKey(secret), mac, data);
}

function concat(...parts: Uint8Array[]): Uint8Array {
  const out = new Uint8Array(parts.reduce((n, p) => n + p.length, 0));
  let o = 0;
  for (const p of parts) {
    out.set(p, o);
    o += p.length;
  }
  return out;
}

function u32(n: number): Uint8Array {
  const b = new Uint8Array(4);
  new DataView(b.buffer).setUint32(0, n);
  return b;
}

async function signed(secret: string, body: Uint8Array): Promise<string> {
  return b64url(concat(body, await sign(secret, body)));
}

async function opened(secret: string, token: string, bodyLen: number): Promise<Uint8Array | null> {
  const raw = fromB64url(token);
  if (!raw || raw.length !== bodyLen + MAC_LEN) return null;
  const body = raw.subarray(0, bodyLen);
  return (await verify(secret, raw.subarray(bodyLen), body)) ? body : null;
}

export interface Challenge {
  id: string;
  expS: number;
  difficulty: number;
}

export async function mintChallenge(secret: string, expS: number, difficulty: number): Promise<string> {
  return signed(secret, concat(random(16), u32(expS), Uint8Array.of(difficulty)));
}

export async function openChallenge(secret: string, token: string): Promise<Challenge | null> {
  const body = await opened(secret, token, 21);
  if (!body) return null;
  const view = new DataView(body.buffer, body.byteOffset, body.byteLength);
  return { id: b64url(body.subarray(0, 16)), expS: view.getUint32(16), difficulty: body[20] };
}

export async function mintInstall(secret: string): Promise<string> {
  return signed(secret, random(16));
}

export async function openInstall(secret: string, token: string): Promise<string | null> {
  const body = await opened(secret, token, 16);
  return body ? b64url(body) : null;
}

export async function mintUpload(secret: string, id: string, expS: number): Promise<string> {
  const exp = u32(expS);
  return b64url(concat(exp, await sign(secret, concat(enc.encode(id), exp))));
}

export async function openUpload(secret: string, id: string, token: string): Promise<number | null> {
  const raw = fromB64url(token);
  if (!raw || raw.length !== 4 + MAC_LEN) return null;
  const exp = raw.subarray(0, 4);
  if (!(await verify(secret, raw.subarray(4), concat(enc.encode(id), exp)))) return null;
  return new DataView(exp.buffer, exp.byteOffset, 4).getUint32(0);
}

export async function mintDelete(secret: string, id: string): Promise<string> {
  return b64url(await sign(secret, enc.encode(id)));
}

export async function deleteOk(secret: string, id: string, token: string): Promise<boolean> {
  const raw = fromB64url(token);
  if (!raw || raw.length !== MAC_LEN) return false;
  return verify(secret, raw, enc.encode(id));
}

export async function hashIp(secret: string, day: string, ip: string): Promise<string> {
  return b64url((await sign(secret, enc.encode(`${day}\n${ip}`))).subarray(0, 16));
}
