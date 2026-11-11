import { env, exports } from "cloudflare:workers";
import { leadingZeroBits, powDigest } from "../src/pow";
import { mintInstall } from "../src/tokens";
import type { Env } from "../src/env";

export const testEnv = env as unknown as Env;
export const worker = (exports as unknown as { default: Fetcher }).default;
export const VERSION = "0.2.0";
export const MiB = 1024 * 1024;

export const PNG = [0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a];
export const JPEG = [0xff, 0xd8, 0xff, 0xe0];
export const MP4 = [0, 0, 0, 0x20, ...[..."ftypisom"].map((c) => c.charCodeAt(0))];

export interface Call {
  token?: string;
  json?: unknown;
  body?: Uint8Array;
  headers?: Record<string, string>;
}

export function call(method: string, path: string, c: Call = {}): Promise<Response> {
  const headers: Record<string, string> = { "X-Easyshot-Version": VERSION, ...c.headers };
  if (c.token) headers.Authorization = `Bearer ${c.token}`;
  let body: BodyInit | undefined;
  if (c.json !== undefined) {
    headers["Content-Type"] = "application/json";
    body = JSON.stringify(c.json);
  } else if (c.body) {
    body = c.body;
  }
  return worker.fetch(`https://share.easyshot.site${path}`, { method, headers, body });
}

export function bytes(len: number, magic: number[] = []): Uint8Array {
  const b = new Uint8Array(len);
  b.fill(0x41);
  b.set(magic.slice(0, len));
  return b;
}

export async function solve(challenge: string, difficulty: number): Promise<string> {
  for (let n = 0; ; n++) {
    if (leadingZeroBits(await powDigest(challenge, String(n))) >= difficulty) return String(n);
  }
}

export async function installViaApi(ip = "192.0.2.1"): Promise<Response> {
  const c = (await (await call("GET", "/v1/challenge")).json()) as { challenge: string; difficulty: number };
  return call("POST", "/v1/install", { json: { challenge: c.challenge, nonce: await solve(c.challenge, c.difficulty) }, headers: { "CF-Connecting-IP": ip } });
}

export function installToken(): Promise<string> {
  return mintInstall(testEnv.INSTALL_KEY);
}

export function image(size: number, extra: Record<string, unknown> = {}) {
  return { kind: "image", mime: "image/png", size, days: 1, title: null, width: 10, height: 10, duration_ms: null, poster_size: null, ...extra };
}

export function video(size: number, posterSize: number, extra: Record<string, unknown> = {}) {
  return { kind: "video", mime: "video/mp4", size, days: 1, title: null, width: 10, height: 10, duration_ms: 1000, poster_size: posterSize, ...extra };
}

export interface Created {
  id: string;
  media_url: string;
  poster_url: string | null;
  upload_token: string;
  delete_token: string;
  parts: number;
  part_bytes: number;
}

export async function createOk(token: string, body: unknown): Promise<Created> {
  const r = await call("POST", "/v1/shares", { token, json: body });
  if (r.status !== 201) throw new Error(`create: ${r.status} ${await r.text()}`);
  return r.json();
}

export function putPart(s: Created, n: number, body: Uint8Array): Promise<Response> {
  return call("PUT", `/v1/shares/${s.id}/parts/${n}`, { token: s.upload_token, body });
}

export function mediaKey(s: Created): string {
  return new URL(s.media_url).pathname.slice(1);
}

export async function errorOf(r: Response): Promise<{ status: number; code: string; retry_after_s?: number }> {
  const b = (await r.json()) as { error: { code: string; retry_after_s?: number } };
  return { status: r.status, code: b.error.code, retry_after_s: b.error.retry_after_s };
}

export async function left(token: string): Promise<{ shares_left: number; byte_days_left: number; accepting: boolean }> {
  return (await call("GET", "/v1/limits", { token })).json();
}
