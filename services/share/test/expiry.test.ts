import { runDurableObjectAlarm } from "cloudflare:test";
import { describe, expect, it, vi } from "vitest";
import { ledger } from "../src/env";
import { utcDay } from "../src/http";
import { EXPIRE_BATCH, EXPIRE_RETRY_MS, type Ledger } from "../src/ledger";
import { call, type Created, expire, inLedger, liveImage, liveVideo, mediaKey, testEnv } from "./helpers";

const DAY_MS = 86_400_000;

const dir = (s: Created) => mediaKey(s).replace(/\/media\.[a-z0-9]+$/, "/");

async function objects(s: Created): Promise<string[]> {
  return (await testEnv.BUCKET.list({ prefix: dir(s) })).objects.map((o) => o.key.slice(dir(s).length)).sort();
}

const exists = (id: string) => inLedger((l) => l.share(id)).then((s) => s !== null);
const expiresOf = (id: string) => inLedger((l) => l.share(id)).then((s) => s?.expires);
const alarmAt = () => inLedger((_, __, state) => state.storage.getAlarm());
const liveAt = (now: number) => inLedger((l) => l.usage(null, now, utcDay(now)).live);
const runAlarm = () => runDurableObjectAlarm(ledger(testEnv));

function withBucket(l: Ledger, bucket: Partial<R2Bucket>): () => void {
  const o = l as unknown as { env: object };
  const real = o.env;
  o.env = new Proxy(real, { get: (t, k) => (k === "BUCKET" ? bucket : Reflect.get(t, k)) });
  return () => {
    o.env = real;
  };
}

describe("expiry", () => {
  it("sets the alarm to the earliest expiry, moving it earlier for a share that expires sooner", async () => {
    const week = await liveImage({ days: 7 });
    expect(await alarmAt()).toBe(await expiresOf(week.id));
    const day = await liveImage({ days: 1 });
    const dayExpires = await expiresOf(day.id);
    expect(dayExpires).toBeLessThan((await expiresOf(week.id)) ?? 0);
    expect(await alarmAt()).toBe(dayExpires);
    await liveImage({ days: 3 });
    expect(await alarmAt()).toBe(dayExpires);
  });

  it("deletes nothing when the alarm fires before any share is due, and sets it again", async () => {
    const s = await liveImage();
    const at = await expiresOf(s.id);
    expect(await runAlarm()).toBe(true);
    expect(await exists(s.id)).toBe(true);
    expect(await objects(s)).toEqual(["media.png"]);
    expect(await alarmAt()).toBe(at);
  });

  it("deletes exactly the due shares, with media, poster and staged poster, and keeps their bytes counted for a day", async () => {
    const video = await liveVideo();
    await testEnv.BUCKET.put(`${dir(video)}poster.upload`, "x");
    const image = await liveImage();
    const notYet = await liveImage();
    const now = Date.now();
    await expire(video.id, now - 1000);
    await expire(image.id, now - 500);
    await expire(notYet.id, now + 60_000);
    expect(await objects(video)).toEqual(["media.mp4", "poster.jpg", "poster.upload"]);
    const live = await liveAt(now);

    expect(await runAlarm()).toBe(true);
    expect(await exists(video.id)).toBe(false);
    expect(await exists(image.id)).toBe(false);
    expect(await objects(video)).toEqual([]);
    expect(await objects(image)).toEqual([]);
    expect(await exists(notYet.id)).toBe(true);
    expect(await objects(notYet)).toEqual(["media.png"]);
    expect(await alarmAt()).toBe(now + 60_000);

    expect(await liveAt(Date.now())).toBe(live);
    expect(await liveAt(now - 500 + DAY_MS)).toBe(live - 20 - 16);
  });

  it("deletes a batch at a time and sets itself again at once while more are due", async () => {
    const now = Date.now();
    const ids = Array.from({ length: EXPIRE_BATCH + 1 }, (_, i) => `1${String(i).padStart(21, "0")}`);
    await inLedger((_, sql) => {
      for (const [i, id] of ids.entries()) {
        sql.exec(
          `INSERT INTO shares (id, install, day, kind, mime, size, poster_size, days, part_bytes, parts, width, height, created, expires, hold_until, state)
           VALUES (?, 'i', '2026-11-11', 'image', 'image/png', 1, 0, 1, 1, 1, 1, 1, 0, ?, ?, 'live')`,
          id, now - 10_000 + i, now + DAY_MS,
        );
      }
    });
    await Promise.all(ids.map((id) => testEnv.BUCKET.put(`s/1d/${id}/media.png`, "x")));
    const left = await inLedger(async (l, sql, state) => {
      await l.alarm();
      return { rows: sql.exec<{ n: number }>("SELECT COUNT(*) AS n FROM shares").one().n, alarm: await state.storage.getAlarm() };
    });
    expect(left.rows).toBe(1);
    expect(left.alarm).not.toBeNull();
    expect(left.alarm).toBeLessThanOrEqual(Date.now());
    await vi.waitFor(async () => {
      expect(await exists(ids[EXPIRE_BATCH])).toBe(false);
      expect((await testEnv.BUCKET.list({ prefix: "s/1d/" })).objects).toEqual([]);
    }, 10_000);
  });

  it("is fine when the owner deleted the share before it expired", async () => {
    const gone = await liveImage();
    const due = await liveImage();
    expect((await call("DELETE", `/v1/shares/${gone.id}`, { token: gone.delete_token })).status).toBe(204);
    await expire(gone.id);
    await expire(due.id);
    expect(await runAlarm()).toBe(true);
    expect(await exists(due.id)).toBe(false);
    expect(await objects(due)).toEqual([]);
  });

  it("gives a share's bytes back once when its owner deletes it while the alarm waits on R2", async () => {
    const raced = await liveImage();
    const due = await liveImage();
    const now = Date.now();
    await expire(raced.id, now - 1000);
    await expire(due.id, now - 1000);
    const live = await liveAt(now);
    await inLedger(async (l) => {
      const restore = withBucket(l, {
        delete: async (keys) => {
          l.remove(raced.id);
          return testEnv.BUCKET.delete(keys);
        },
      });
      try {
        await l.alarm();
      } finally {
        restore();
      }
    });
    expect(await exists(raced.id)).toBe(false);
    expect(await exists(due.id)).toBe(false);
    expect(await liveAt(now)).toBe(live - 16);
    expect(await liveAt(now + DAY_MS)).toBe(live - 32);
  });

  it("keeps a share and its files when the R2 delete fails, and deletes them on the next alarm", async () => {
    const s = await liveImage();
    await expire(s.id);
    const armed = await inLedger(async (l, _, state) => {
      const restore = withBucket(l, {
        delete: async () => {
          throw new Error("R2 is down");
        },
      });
      try {
        await l.alarm();
      } finally {
        restore();
      }
      return (await state.storage.getAlarm()) ?? 0;
    });
    expect(await exists(s.id)).toBe(true);
    expect(await objects(s)).toEqual(["media.png"]);
    expect(armed - Date.now()).toBeGreaterThan(EXPIRE_RETRY_MS - 5000);
    expect(armed - Date.now()).toBeLessThanOrEqual(EXPIRE_RETRY_MS);

    expect(await runAlarm()).toBe(true);
    expect(await exists(s.id)).toBe(false);
    expect(await objects(s)).toEqual([]);
    expect((await call("GET", `/${s.id}`)).status).toBe(404);
  });
});
