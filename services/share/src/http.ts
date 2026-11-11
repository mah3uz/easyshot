export const STATUS = {
  bad_request: 400,
  unauthorized: 401,
  token_expired: 401,
  challenge_failed: 403,
  origin_refused: 403,
  not_found: 404,
  conflict: 409,
  too_large: 413,
  unsupported_type: 415,
  version_unsupported: 426,
  rate_limited: 429,
  quota_daily: 429,
  full: 503,
} as const;

export type Code = keyof typeof STATUS;

const JSON_TYPE = "application/json; charset=utf-8";

export function json(body: unknown, status = 200): Response {
  return new Response(JSON.stringify(body), { status, headers: { "Content-Type": JSON_TYPE } });
}

export function fail(code: Code, message: string, retryAfterS?: number): Response {
  const error: Record<string, unknown> = { code, message };
  if (retryAfterS !== undefined) error.retry_after_s = Math.max(1, Math.ceil(retryAfterS));
  return json({ error }, STATUS[code]);
}

export function bearer(req: Request): string | null {
  const h = req.headers.get("Authorization");
  if (!h || !h.startsWith("Bearer ")) return null;
  const t = h.slice(7);
  return t.length > 0 && t.length <= 512 ? t : null;
}

function semver(v: string): [number, number, number] | null {
  const m = /^(0|[1-9]\d{0,8})\.(0|[1-9]\d{0,8})\.(0|[1-9]\d{0,8})(?:[-+][0-9A-Za-z.+-]*)?$/.exec(v);
  return m ? [Number(m[1]), Number(m[2]), Number(m[3])] : null;
}

export function versionOk(req: Request, min: string): boolean {
  const have = semver(req.headers.get("X-Easyshot-Version") ?? "");
  const need = semver(min);
  if (!need) throw new Error("MIN_APP_VERSION is not a version");
  if (!have) return false;
  for (let i = 0; i < 3; i++) {
    if (have[i] !== need[i]) return have[i] > need[i];
  }
  return true;
}

export async function readJson(req: Request, maxBytes = 4096): Promise<unknown> {
  const len = req.headers.get("Content-Length");
  if (len !== null && Number(len) > maxBytes) return undefined;
  const text = await req.text();
  if (text.length > maxBytes) return undefined;
  try {
    return JSON.parse(text);
  } catch {
    return undefined;
  }
}

export function utcDay(nowMs: number): string {
  return new Date(nowMs).toISOString().slice(0, 10);
}

export function secondsToMidnight(nowMs: number): number {
  const d = new Date(nowMs);
  const next = Date.UTC(d.getUTCFullYear(), d.getUTCMonth(), d.getUTCDate() + 1);
  return Math.ceil((next - nowMs) / 1000);
}

export function rfc3339(ms: number): string {
  return new Date(Math.floor(ms / 1000) * 1000).toISOString().replace(".000Z", "Z");
}
