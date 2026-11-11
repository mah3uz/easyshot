import { BURST_PERIOD_S, DAYS, type Env, ledger, limits, UPLOAD_TTL_S } from "./env";
import { bearer, fail, json, readJson, rfc3339, secondsToMidnight, utcDay, versionOk } from "./http";
import type { Share } from "./ledger";
import { type Kind, MAGIC_LEN, MIMES, mediaHttp, POSTER_HTTP, magicOk, mediaKey, posterKey, posterUploadKey, shareKeys } from "./media";
import { b64url, deleteOk, mintDelete, mintUpload, openInstall, openUpload, random } from "./tokens";

const DAY_MS = 86_400_000;

const tooOld = () => fail("version_unsupported", "app version below the minimum");
const notFound = () => fail("not_found", "no such share");

interface CreateBody {
  kind: string;
  mime: string;
  size: number;
  days: number;
  title: string | null;
  width: number;
  height: number;
  duration_ms: number | null;
  poster_size: number | null;
}

const int = (v: unknown, min: number, max = Number.MAX_SAFE_INTEGER): v is number =>
  typeof v === "number" && Number.isSafeInteger(v) && v >= min && v <= max;

const intOrNull = (v: unknown, min: number) => v === null || v === undefined || int(v, min);

function parseCreate(b: unknown): CreateBody | string {
  if (typeof b !== "object" || b === null || Array.isArray(b)) return "body must be a JSON object";
  const o = b as Record<string, unknown>;
  if (typeof o.kind !== "string" || typeof o.mime !== "string") return "kind and mime must be strings";
  if (!int(o.size, 1)) return "size must be a positive integer";
  if (!int(o.days, 0)) return "days must be an integer";
  if (!int(o.width, 1, 65535) || !int(o.height, 1, 65535)) return "width and height must be 1 to 65535";
  const title = o.title ?? null;
  if (title !== null && (typeof title !== "string" || [...title].length > 120)) return "title must be at most 120 characters or null";
  if (!intOrNull(o.duration_ms, 0) || !intOrNull(o.poster_size, 1)) return "duration_ms and poster_size must be integers or null";
  return {
    kind: o.kind,
    mime: o.mime,
    size: o.size,
    days: o.days,
    title,
    width: o.width,
    height: o.height,
    duration_ms: (o.duration_ms as number | null | undefined) ?? null,
    poster_size: (o.poster_size as number | null | undefined) ?? null,
  };
}

export function urls(env: Env, s: { id: string; days: number; mime: string; kind: string }) {
  return {
    page_url: `${env.PAGE_ORIGIN}/${s.id}`,
    media_url: `${env.MEDIA_ORIGIN}/${mediaKey(s.id, s.days, s.mime)}`,
    poster_url: s.kind === "video" ? `${env.MEDIA_ORIGIN}/${posterKey(s.id, s.days)}` : null,
  };
}

function done(env: Env, s: Share) {
  return { ...urls(env, s), expires_at: rfc3339(s.expires) };
}

async function purge(env: Env, s: Share): Promise<void> {
  await env.BUCKET.delete(shareKeys(s.id, s.days, s.mime));
}

function exactBody(req: Request, len: number): ReadableStream | null {
  if (req.headers.get("Content-Length") !== String(len) || !req.body) return null;
  const fixed = new FixedLengthStream(len);
  // A short or long body errors the fixed stream, which fails the R2 write; nothing to handle here.
  req.body.pipeTo(fixed.writable).catch(() => {});
  return fixed.readable;
}

async function uploadAuth(req: Request, env: Env, id: string): Promise<Response | null> {
  const t = bearer(req);
  const exp = t ? await openUpload(env.UPLOAD_KEY, id, t) : null;
  if (exp === null) return fail("unauthorized", "upload token missing or not valid for this share");
  if (exp * 1000 <= Date.now()) return fail("token_expired", "upload token expired");
  return versionOk(req, env.MIN_APP_VERSION) ? null : tooOld();
}

export async function create(req: Request, env: Env): Promise<Response> {
  const t = bearer(req);
  const install = t ? await openInstall(env.INSTALL_KEY, t) : null;
  if (!install) return fail("unauthorized", "install token missing or not valid");
  if (!versionOk(req, env.MIN_APP_VERSION)) return tooOld();
  if (!(await env.BURST.limit({ key: install })).success) return fail("rate_limited", "too many shares in a short time", BURST_PERIOD_S);

  const p = parseCreate(await readJson(req));
  if (typeof p === "string") return fail("bad_request", p);
  const type = MIMES[p.mime];
  if (!type || type.kind !== p.kind) return fail("unsupported_type", `${p.kind} ${p.mime} is not accepted`);
  const kind: Kind = type.kind;
  if (kind === "video" ? p.duration_ms === null || p.poster_size === null : p.duration_ms !== null || p.poster_size !== null) {
    return fail("bad_request", "duration_ms and poster_size are required for video and null for images");
  }
  const l = limits(env);
  const cap = kind === "video" ? l.videoMax : p.mime === "image/gif" ? l.gifMax : l.imageMax;
  if (p.size > cap) return fail("too_large", `${p.mime === "image/gif" ? "gif" : kind} over ${cap} bytes`);
  if ((p.poster_size ?? 0) > l.posterMax) return fail("too_large", `poster over ${l.posterMax} bytes`);
  if (!(DAYS as readonly number[]).includes(p.days)) return fail("bad_request", `days must be one of ${DAYS.join(", ")}`);

  const now = Date.now();
  const created = Math.floor(now / 1000) * 1000;
  const id = `${p.days}${b64url(random(16)).slice(0, 21)}`;
  const share = {
    id,
    install,
    day: utcDay(now),
    kind,
    mime: p.mime,
    size: p.size,
    poster_size: p.poster_size ?? 0,
    days: p.days,
    part_bytes: l.partBytes,
    parts: Math.ceil(p.size / l.partBytes),
    title: p.title,
    width: p.width,
    height: p.height,
    duration_ms: p.duration_ms,
    created,
    expires: created + p.days * DAY_MS,
  };
  const stub = ledger(env);
  const r = await stub.reserve(share, l);
  if (!r.ok) {
    return r.error === "full" ? fail("full", "storage stop", r.retryAfterS) : fail("quota_daily", "daily shares or byte-days spent", secondsToMidnight(now));
  }
  let uploadId: string;
  try {
    const mpu = await env.BUCKET.createMultipartUpload(mediaKey(id, p.days, p.mime), { httpMetadata: mediaHttp(id, p.mime) });
    uploadId = mpu.uploadId;
  } catch (e) {
    await stub.remove(id);
    throw e;
  }
  await stub.setUpload(id, uploadId);
  const uploadExpS = Math.floor(now / 1000) + UPLOAD_TTL_S;
  return json(
    {
      id,
      ...urls(env, share),
      expires_at: rfc3339(share.expires),
      upload_token: await mintUpload(env.UPLOAD_KEY, id, uploadExpS),
      upload_expires_at: rfc3339(uploadExpS * 1000),
      part_bytes: share.part_bytes,
      parts: share.parts,
      delete_token: await mintDelete(env.DELETE_KEY, id),
    },
    201,
  );
}

export async function putPart(req: Request, env: Env, id: string, n: number): Promise<Response> {
  const denied = await uploadAuth(req, env, id);
  if (denied) return denied;
  const stub = ledger(env);
  const s = await stub.share(id);
  if (!s || !s.upload_id) return notFound();
  if (s.state !== "pending") return fail("conflict", "the upload is already complete");
  if (n > s.parts) return fail("bad_request", `the share has ${s.parts} parts`);
  const len = n < s.parts ? s.part_bytes : s.size - (s.parts - 1) * s.part_bytes;
  const body = exactBody(req, len);
  if (!body) return fail("conflict", `part ${n} must be exactly ${len} bytes`);
  let etag: string;
  try {
    etag = (await env.BUCKET.resumeMultipartUpload(mediaKey(id, s.days, s.mime), s.upload_id).uploadPart(n, body)).etag;
  } catch (e) {
    if (!(await stub.share(id))) return notFound();
    throw e;
  }
  const r = await stub.recordPart(id, n, etag);
  if (r === "gone") return notFound();
  if (r === "closed") return fail("conflict", "the upload is already complete");
  return json({ part: n });
}

export async function putPoster(req: Request, env: Env, id: string): Promise<Response> {
  const denied = await uploadAuth(req, env, id);
  if (denied) return denied;
  const stub = ledger(env);
  const s = await stub.share(id);
  if (!s) return notFound();
  if (s.kind !== "video") return fail("bad_request", "only a video has a poster");
  if (s.state !== "pending") return fail("conflict", "the upload is already complete");
  const type = (req.headers.get("Content-Type") ?? "").split(";")[0].trim().toLowerCase();
  if (type !== "image/jpeg") return fail("unsupported_type", "the poster must be image/jpeg");
  const body = exactBody(req, s.poster_size);
  if (!body) return fail("conflict", `the poster must be exactly ${s.poster_size} bytes`);
  // Only complete writes poster.jpg, from bytes it read and checked, so a PUT racing it cannot swap them.
  const key = posterUploadKey(id, s.days);
  await env.BUCKET.put(key, body, { httpMetadata: { contentType: "application/octet-stream", contentDisposition: "attachment" } });
  const r = await stub.recordPoster(id);
  if (r === "gone") {
    await env.BUCKET.delete(key);
    return notFound();
  }
  if (r === "closed") return fail("conflict", "the upload is already complete");
  return json({});
}

async function head(env: Env, key: string, len: number): Promise<Uint8Array | null> {
  const o = await env.BUCKET.get(key, { range: { offset: 0, length: len } });
  return o ? new Uint8Array(await o.arrayBuffer()) : null;
}

export async function complete(req: Request, env: Env, id: string): Promise<Response> {
  const denied = await uploadAuth(req, env, id);
  if (denied) return denied;
  const stub = ledger(env);
  const b = await stub.beginComplete(id);
  if (!b.ok) {
    if (b.why === "gone") return notFound();
    if (b.why === "live" && b.share) return json(done(env, b.share));
    return fail("conflict", b.why === "busy" ? "complete is already running" : "not every part has arrived");
  }
  const s = b.share;
  const key = mediaKey(id, s.days, s.mime);
  try {
    await env.BUCKET.resumeMultipartUpload(key, s.upload_id ?? "").complete(b.parts);
  } catch {
    // The join may have succeeded with the reply lost; the object's presence decides.
    if (!(await env.BUCKET.head(key))) {
      await stub.reopen(id);
      return (await stub.share(id)) ? fail("conflict", "the parts could not be joined") : notFound();
    }
  }
  const media = await head(env, key, MAGIC_LEN);
  let ok = media !== null && magicOk(s.mime, media);
  if (ok && s.kind === "video") {
    const staged = await env.BUCKET.get(posterUploadKey(id, s.days));
    const poster = staged ? new Uint8Array(await staged.arrayBuffer()) : null;
    ok = poster !== null && poster.length === s.poster_size && magicOk("image/jpeg", poster);
    if (ok && poster) {
      await env.BUCKET.put(posterKey(id, s.days), poster, { httpMetadata: POSTER_HTTP });
      await env.BUCKET.delete(posterUploadKey(id, s.days));
    }
  }
  if (!ok) {
    const existed = await stub.remove(id);
    await purge(env, s);
    return existed ? fail("unsupported_type", `the bytes are not ${s.mime}`) : notFound();
  }
  if (!(await stub.finishComplete(id))) {
    await purge(env, s);
    return notFound();
  }
  return json(done(env, s));
}

export async function remove(req: Request, env: Env, id: string): Promise<Response> {
  const t = bearer(req);
  if (!t || !(await deleteOk(env.DELETE_KEY, id, t))) return fail("unauthorized", "delete token missing or not valid for this share");
  if (!versionOk(req, env.MIN_APP_VERSION)) return tooOld();
  const s = await ledger(env).remove(id);
  if (!s) return notFound();
  if (s.state !== "live" && s.upload_id) {
    try {
      await env.BUCKET.resumeMultipartUpload(mediaKey(id, s.days, s.mime), s.upload_id).abort();
    } catch {
      // Already completed or aborted; the delete below removes whatever exists.
    }
  }
  await purge(env, s);
  return new Response(null, { status: 204 });
}

export const SHARE_ID = /^[137][A-Za-z0-9_-]{21}$/;
