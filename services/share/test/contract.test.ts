import { describe, expect, it } from "vitest";
import api from "../contract/api.json";
import errors from "../contract/errors.json";
import pow from "../contract/pow.json";
import { fail, STATUS, type Code } from "../src/http";
import { leadingZeroBits, powDigest, powOk } from "../src/pow";
import { bytes, call, createOk, image, installToken, JPEG, mediaKey, MP4, PNG, putPart, solve, testEnv, worker } from "./helpers";

const ID = /^[137][A-Za-z0-9_-]{21}$/;
const TIME = /^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}Z$/;
const SUBSTITUTED = ["challenge", "install_token", "upload_token", "delete_token", "id"];

function shape(v: unknown): unknown {
  if (v === null) return "null";
  if (Array.isArray(v)) return v.map(shape);
  if (typeof v === "object") return Object.fromEntries(Object.entries(v as object).map(([k, x]) => [k, shape(x)]).sort());
  return typeof v;
}

const hex = (b: Uint8Array) => [...b].map((x) => x.toString(16).padStart(2, "0")).join("");

describe("error codes", () => {
  it("map to the statuses the contract gives", () => {
    expect(STATUS).toEqual(errors.codes);
  });

  it("produce the contract's error bodies", async () => {
    for (const ex of errors.examples) {
      const e = ex.body.error as { code: Code; message: string; retry_after_s?: number };
      const r = fail(e.code, e.message, e.retry_after_s);
      expect(r.status).toBe(ex.status);
      expect(await r.json()).toEqual(ex.body);
      expect(r.headers.get("Access-Control-Allow-Origin")).toBeNull();
    }
  });
});

describe("proof of work", () => {
  it("matches every vector, and each nonce is the smallest that meets its difficulty", async () => {
    for (const c of pow.cases) {
      const d = await powDigest(c.challenge, c.nonce);
      expect(hex(d)).toBe(c.sha256);
      expect(leadingZeroBits(d)).toBeGreaterThanOrEqual(c.difficulty);
      expect(await powOk(c.challenge, c.nonce, c.difficulty)).toBe(true);
      expect(await solve(c.challenge, c.difficulty)).toBe(c.nonce);
    }
  });

  it("refuses a nonce one short of the vector, and non-decimal nonces", async () => {
    const c = pow.cases[2];
    expect(await powOk(c.challenge, String(Number(c.nonce) - 1), c.difficulty)).toBe(false);
    expect(await powOk(c.challenge, `0${c.nonce}`, c.difficulty)).toBe(false);
    expect(await powOk(c.challenge, ` ${c.nonce}`, c.difficulty)).toBe(false);
  });
});

describe("api exchanges", () => {
  it("answer every fixture request with its status and body shape", async () => {
    const subs = new Map<string, string>();
    const sub = (s: string) => [...subs].reduce((acc, [k, v]) => acc.split(k).join(v), s);
    const got: Record<string, Record<string, unknown>> = {};
    const send = (method: string, path: string, headers: Record<string, string>, body?: BodyInit) =>
      worker.fetch(`https://share.easyshot.site${path}`, { method, headers, body });

    for (const ex of api.exchanges) {
      const req = ex.request as unknown as { method: string; path: string; headers: Record<string, string>; body?: Record<string, unknown> };
      const headers = Object.fromEntries(Object.entries(req.headers).filter(([k]) => k !== "Content-Length").map(([k, v]) => [k, sub(v)]));
      const path = sub(req.path);
      let body: BodyInit | undefined;
      if (ex.name === "install") {
        const challenge = sub(String(req.body?.challenge));
        body = JSON.stringify({ challenge, nonce: await solve(challenge, Number(got.challenge.difficulty)) });
      } else if (req.body) {
        body = JSON.stringify(req.body);
      } else if (req.headers["Content-Length"]) {
        body = bytes(Number(req.headers["Content-Length"]), ex.name === "poster" ? JPEG : []);
      }
      if (ex.name === "part_last") {
        const partBytes = Number(got.create_video.part_bytes);
        for (const n of [1, 2]) {
          const r = await send("PUT", path.replace(/\/3$/, `/${n}`), headers, bytes(partBytes, n === 1 ? MP4 : []));
          expect(r.status, `part ${n}`).toBe(200);
        }
      }

      const r = await send(req.method, path, headers, body);
      expect(r.status, ex.name).toBe(ex.response.status);
      expect(r.headers.get("Access-Control-Allow-Origin"), ex.name).toBeNull();
      if (!("body" in ex.response)) {
        expect(await r.text(), ex.name).toBe("");
        continue;
      }
      const b = (await r.json()) as Record<string, unknown>;
      expect(shape(b), ex.name).toEqual(shape(ex.response.body));
      got[ex.name] = b;
      const fixture = ex.response.body as Record<string, unknown>;
      for (const k of SUBSTITUTED) {
        if (typeof fixture[k] === "string" && typeof b[k] === "string") subs.set(fixture[k] as string, b[k] as string);
      }
    }

    expect(got.challenge.difficulty).toBe(Number(testEnv.POW_DIFFICULTY));
    expect(got.challenge.expires_at).toMatch(TIME);

    const limitsFixture = api.exchanges.find((e) => e.name === "limits")?.response.body as Record<string, unknown>;
    for (const k of ["image_max_bytes", "gif_max_bytes", "video_max_bytes", "poster_max_bytes", "days", "default_days", "part_bytes", "daily_byte_days"]) {
      expect(got.limits[k], k).toEqual(limitsFixture[k]);
    }
    expect(got.limits.shares_left).toBe(Number(testEnv.DAILY_SHARES));
    expect(got.limits.accepting).toBe(true);

    for (const [name, days, ext, parts] of [["create_image", 7, "png", 1], ["create_video", 3, "mp4", 3]] as const) {
      const c = got[name];
      expect(c.id).toMatch(ID);
      expect(String(c.id)[0]).toBe(String(days));
      expect(c.page_url).toBe(`https://share.easyshot.site/${c.id}`);
      expect(c.media_url).toBe(`https://m.easyshot.site/s/${days}d/${c.id}/media.${ext}`);
      expect(c.parts).toBe(parts);
      expect(c.part_bytes).toBe(26214400);
      expect(c.expires_at).toMatch(TIME);
      const ttl = Date.parse(String(c.expires_at)) - Date.now();
      expect(Math.abs(ttl - days * 86_400_000)).toBeLessThan(5_000);
      const upload = Date.parse(String(c.upload_expires_at)) - Date.now();
      expect(Math.abs(upload - 6 * 3_600_000)).toBeLessThan(5_000);
    }
    expect(got.create_image.poster_url).toBeNull();
    expect(got.create_video.poster_url).toBe(`https://m.easyshot.site/s/3d/${got.create_video.id}/poster.jpg`);

    const { page_url, media_url, poster_url, expires_at } = got.create_video;
    expect(got.complete).toEqual({ page_url, media_url, poster_url, expires_at });
    expect(got.origin_refused).toEqual(api.exchanges.find((e) => e.name === "origin_refused")?.response.body);

    const dir = `s/3d/${got.create_video.id}`;
    expect(await testEnv.BUCKET.head(`${dir}/media.mp4`)).toBeNull();
    expect(await testEnv.BUCKET.head(`${dir}/poster.jpg`)).toBeNull();
  });

  it("stores completed media with its type, cache and its download name in the disposition", async () => {
    const s = await createOk(await installToken(), image(100, { days: 3 }));
    expect((await putPart(s, 1, bytes(100, PNG))).status).toBe(200);
    expect((await call("POST", `/v1/shares/${s.id}/complete`, { token: s.upload_token })).status).toBe(200);
    const o = await testEnv.BUCKET.head(mediaKey(s));
    expect(o?.size).toBe(100);
    expect(o?.httpMetadata).toMatchObject({
      contentType: "image/png",
      cacheControl: "public, max-age=3600",
      contentDisposition: `inline; filename="easyshot-${s.id}.png"`,
    });
  });
});
