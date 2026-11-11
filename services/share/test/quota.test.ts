import { describe, expect, it } from "vitest";
import { call, createOk, errorOf, image, installToken, left, MiB, testEnv, video } from "./helpers";

const STOP = Number(testEnv.STORAGE_STOP_BYTES);

function toMidnight(): number {
  const d = new Date();
  return (Date.UTC(d.getUTCFullYear(), d.getUTCMonth(), d.getUTCDate() + 1) - d.getTime()) / 1000;
}

const post = (token: string, body: unknown) => call("POST", "/v1/shares", { token, json: body });

describe("daily quota", () => {
  it("stops an install at its daily share count until 00:00 UTC", async () => {
    const t = await installToken();
    const daily = Number(testEnv.DAILY_SHARES);
    for (let i = 0; i < daily; i++) await createOk(t, image(10));
    expect((await left(t)).shares_left).toBe(0);
    const e = await errorOf(await post(t, image(10)));
    expect(e).toMatchObject({ status: 429, code: "quota_daily" });
    expect(Math.abs((e.retry_after_s ?? 0) - toMidnight())).toBeLessThan(3);
    expect((await post(await installToken(), image(10))).status).toBe(201);
  });

  it("stops an install whose byte-days are spent, counting the poster and the days", async () => {
    const t = await installToken();
    const budget = Number(testEnv.DAILY_BYTE_DAYS);
    await createOk(t, video(199 * MiB, 100 * 1024, { days: 7 }));
    const rest = (await left(t)).byte_days_left;
    expect(rest).toBe(budget - (199 * MiB + 100 * 1024) * 7);
    expect(await errorOf(await post(t, image(Math.floor(rest / 7) + 1, { days: 7 })))).toMatchObject({ status: 429, code: "quota_daily" });
    expect((await post(t, image(Math.floor(rest / 7), { days: 7 }))).status).toBe(201);
    const e = await errorOf(await post(t, image(1)));
    expect(e).toMatchObject({ status: 429, code: "quota_daily" });
    expect(e.retry_after_s).toBeGreaterThan(0);
  });
});

describe("burst limit", () => {
  it("answers rate_limited with retry_after_s after five creates a minute from one install", async () => {
    const t = await installToken();
    const daily = Number(testEnv.DAILY_SHARES);
    const codes: number[] = [];
    for (let i = 0; i < 5; i++) codes.push((await post(t, image(10))).status);
    expect(codes).toEqual(Array.from({ length: 5 }, (_, i) => (i < daily ? 201 : 429)));
    const e = await errorOf(await post(t, image(10)));
    expect(e).toMatchObject({ status: 429, code: "rate_limited", retry_after_s: 60 });
  });
});

describe("storage stop", () => {
  it("refuses new shares with full once live bytes reach the stop, and accepts again after a delete", async () => {
    const a = await createOk(await installToken(), video(200 * MiB, 1));
    await createOk(await installToken(), video(STOP - 200 * MiB - 2, 1));
    expect((await left(await installToken())).accepting).toBe(false);
    const e = await errorOf(await post(await installToken(), image(1)));
    expect(e).toMatchObject({ status: 503, code: "full" });
    expect(Math.abs((e.retry_after_s ?? 0) - 2 * 86_400)).toBeLessThan(3);

    expect((await call("DELETE", `/v1/shares/${a.id}`, { token: a.delete_token })).status).toBe(204);
    expect((await left(await installToken())).accepting).toBe(true);
    expect((await post(await installToken(), image(1))).status).toBe(201);
  });

  it("cannot be overshot by creates racing in parallel", async () => {
    const size = 10 * MiB;
    const fits = Math.floor(STOP / size);
    const tokens = await Promise.all(Array.from({ length: fits + 15 }, installToken));
    const results = await Promise.all(tokens.map((t) => post(t, image(size))));
    const statuses = results.map((r) => r.status);
    expect(statuses.filter((s) => s === 201).length).toBe(fits);
    expect(statuses.filter((s) => s === 503).length).toBe(15);
    expect((await left(await installToken())).accepting).toBe(false);
  });
});
