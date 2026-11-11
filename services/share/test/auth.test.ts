import { describe, expect, it } from "vitest";
import { powOk } from "../src/pow";
import { mintChallenge, mintInstall, mintUpload } from "../src/tokens";
import { call, createOk, errorOf, image, installToken, installViaApi, solve, testEnv, worker } from "./helpers";

async function freshChallenge(): Promise<{ challenge: string; difficulty: number }> {
  return (await call("GET", "/v1/challenge")).json();
}

describe("install", () => {
  it("refuses a challenge used a second time, so one proof of work buys one install", async () => {
    const c = await freshChallenge();
    const body = { challenge: c.challenge, nonce: await solve(c.challenge, c.difficulty) };
    const first = await call("POST", "/v1/install", { json: body, headers: { "CF-Connecting-IP": "192.0.2.10" } });
    expect(first.status).toBe(200);
    const again = await call("POST", "/v1/install", { json: body, headers: { "CF-Connecting-IP": "192.0.2.11" } });
    expect(await errorOf(again)).toMatchObject({ status: 403, code: "challenge_failed" });
  });

  it("refuses a wrong nonce, an expired challenge and one signed with another key", async () => {
    const c = await freshChallenge();
    let nonce = 0;
    while (await powOk(c.challenge, String(nonce), c.difficulty)) nonce++;
    expect(await errorOf(await call("POST", "/v1/install", { json: { challenge: c.challenge, nonce: String(nonce) } }))).toMatchObject({ status: 403, code: "challenge_failed" });

    const expired = await mintChallenge(testEnv.CHALLENGE_KEY, Math.floor(Date.now() / 1000) - 1, 1);
    expect(await errorOf(await call("POST", "/v1/install", { json: { challenge: expired, nonce: await solve(expired, 1) } }))).toMatchObject({ status: 403, code: "challenge_failed" });

    const forged = await mintChallenge("another-key-that-is-at-least-32-chars", Math.floor(Date.now() / 1000) + 60, 0);
    expect(await errorOf(await call("POST", "/v1/install", { json: { challenge: forged, nonce: "0" } }))).toMatchObject({ status: 403, code: "challenge_failed" });
  });

  it("keeps the difficulty the challenge was signed with", async () => {
    const easy = await mintChallenge(testEnv.CHALLENGE_KEY, Math.floor(Date.now() / 1000) + 60, 0);
    const tampered = easy.slice(0, 27) + (easy[27] === "A" ? "B" : "A") + easy.slice(28);
    expect(await errorOf(await call("POST", "/v1/install", { json: { challenge: tampered, nonce: "0" } }))).toMatchObject({ code: "challenge_failed" });
  });

  it("allows five installs per address per UTC day, then rate_limited until midnight", async () => {
    for (let i = 0; i < 5; i++) expect((await installViaApi("198.51.100.7")).status).toBe(200);
    const e = await errorOf(await installViaApi("198.51.100.7"));
    expect(e).toMatchObject({ status: 429, code: "rate_limited" });
    expect(e.retry_after_s).toBeGreaterThan(0);
    expect(e.retry_after_s).toBeLessThanOrEqual(86_400);
    expect((await installViaApi("198.51.100.8")).status).toBe(200);
  });

  it("answers malformed bodies with bad_request", async () => {
    expect(await errorOf(await call("POST", "/v1/install", { json: { challenge: 1 } }))).toMatchObject({ status: 400, code: "bad_request" });
    const r = await call("POST", "/v1/install", { body: new TextEncoder().encode("{"), headers: { "Content-Type": "application/json" } });
    expect(await errorOf(r)).toMatchObject({ status: 400, code: "bad_request" });
  });
});

describe("every /v1 request", () => {
  it("is refused with an Origin header, before anything else, and carries no CORS headers", async () => {
    for (const [method, path] of [["GET", "/v1/challenge"], ["GET", "/v1/limits"], ["POST", "/v1/install"], ["DELETE", "/v1/shares/1aaaaaaaaaaaaaaaaaaaaa"], ["OPTIONS", "/v1/shares"]]) {
      const r = await call(method, path, { headers: { Origin: "https://evil.example" } });
      expect(await errorOf(r), path).toMatchObject({ status: 403, code: "origin_refused" });
      for (const h of r.headers.keys()) expect(h.toLowerCase().startsWith("access-control-"), h).toBe(false);
    }
  });

  it("needs a version header at or above the minimum", async () => {
    for (const v of [undefined, "", "0.1.9", "garbage", "0.2"]) {
      const headers: Record<string, string> = v === undefined ? {} : { "X-Easyshot-Version": v };
      const r = await worker.fetch("https://share.easyshot.site/v1/challenge", { headers });
      expect(await errorOf(r), String(v)).toMatchObject({ status: 426, code: "version_unsupported" });
    }
    for (const v of ["0.2.0", "0.2.1", "0.10.0", "1.0.0"]) {
      expect((await call("GET", "/v1/challenge", { headers: { "X-Easyshot-Version": v } })).status, v).toBe(200);
    }
  });

  it("answers unknown paths and the share page with not_found", async () => {
    expect(await errorOf(await call("GET", "/v1/nothing"))).toMatchObject({ status: 404, code: "not_found" });
    expect((await call("GET", "/7Kp2mX9qR4tY8wZ1aB3cD5")).status).toBe(404);
  });
});

describe("tokens", () => {
  it("refuse a missing, malformed or foreign install token on create", async () => {
    const foreign = await mintInstall("some-other-install-key-0123456789abc");
    for (const token of [undefined, "x", foreign]) {
      expect(await errorOf(await call("POST", "/v1/shares", { token, json: image(10) }))).toMatchObject({ status: 401, code: "unauthorized" });
    }
  });

  it("bind an upload token to its share and its time", async () => {
    const t = await installToken();
    const a = await createOk(t, image(10));
    const b = await createOk(t, image(10));
    const crossed = await call("PUT", `/v1/shares/${b.id}/parts/1`, { token: a.upload_token, body: new Uint8Array(10) });
    expect(await errorOf(crossed)).toMatchObject({ status: 401, code: "unauthorized" });

    const old = await mintUpload(testEnv.UPLOAD_KEY, a.id, Math.floor(Date.now() / 1000) - 1);
    expect(await errorOf(await call("POST", `/v1/shares/${a.id}/complete`, { token: old }))).toMatchObject({ status: 401, code: "token_expired" });
  });

  it("let only the share's delete token delete it", async () => {
    const t = await installToken();
    const a = await createOk(t, image(10));
    const b = await createOk(t, image(10));
    expect(await errorOf(await call("DELETE", `/v1/shares/${a.id}`, { token: b.delete_token }))).toMatchObject({ status: 401, code: "unauthorized" });
    expect(await errorOf(await call("DELETE", `/v1/shares/${a.id}`, { token: a.upload_token }))).toMatchObject({ status: 401, code: "unauthorized" });
    expect((await call("DELETE", `/v1/shares/${a.id}`, { token: a.delete_token })).status).toBe(204);
    expect(await errorOf(await call("DELETE", `/v1/shares/${a.id}`, { token: a.delete_token }))).toMatchObject({ status: 404, code: "not_found" });
  });

  it("show quota only with a valid install token", async () => {
    const anon = (await (await call("GET", "/v1/limits")).json()) as Record<string, unknown>;
    expect(anon).not.toHaveProperty("shares_left");
    expect(anon).not.toHaveProperty("byte_days_left");
    expect(anon.accepting).toBe(true);
  });
});
