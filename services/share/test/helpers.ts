import { runInDurableObject } from "cloudflare:test";
import { env, exports } from "cloudflare:workers";
import { type Env, ledger } from "../src/env";
import type { Ledger } from "../src/ledger";
import { leadingZeroBits, powDigest } from "../src/pow";
import { mintInstall } from "../src/tokens";

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

export async function liveImage(extra: Record<string, unknown> = {}): Promise<Created> {
  const s = await createOk(await installToken(), image(16, extra));
  await putPart(s, 1, bytes(16, PNG));
  await call("POST", `/v1/shares/${s.id}/complete`, { token: s.upload_token });
  return s;
}

export async function liveVideo(extra: Record<string, unknown> = {}): Promise<Created> {
  const s = await createOk(await installToken(), video(16, 4, { title: "Demo", days: 3, ...extra }));
  await putPart(s, 1, bytes(16, MP4));
  await call("PUT", `/v1/shares/${s.id}/poster`, { token: s.upload_token, body: Uint8Array.of(...JPEG.slice(0, 3), 1), headers: { "Content-Type": "image/jpeg" } });
  await call("POST", `/v1/shares/${s.id}/complete`, { token: s.upload_token });
  return s;
}

export function inLedger<R>(f: (l: Ledger, sql: SqlStorage, state: DurableObjectState) => R | Promise<R>): Promise<R> {
  return runInDurableObject(ledger(testEnv), (l: Ledger, state) => f(l, (l as unknown as { sql: SqlStorage }).sql, state));
}

export function expire(id: string, at = Date.now() - 1000): Promise<void> {
  return inLedger((_, sql) => {
    sql.exec("UPDATE shares SET expires = ?, hold_until = ? WHERE id = ?", at, at + 86_400_000, id);
  });
}
