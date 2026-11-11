import { runInDurableObject } from "cloudflare:test";
import { describe, expect, it } from "vitest";
import { ledger } from "../src/env";
import type { Ledger } from "../src/ledger";
import { bytes, call, createOk, errorOf, image, installToken, JPEG, left, mediaKey, MiB, MP4, PNG, putPart, testEnv, video } from "./helpers";

const PART = 26214400;

describe("create", () => {
  it("checks type, then size, then days", async () => {
    const post = async (body: unknown) => call("POST", "/v1/shares", { token: await installToken(), json: body });
    expect(await errorOf(await post(image(10, { mime: "image/svg+xml" })))).toMatchObject({ status: 415, code: "unsupported_type" });
    expect(await errorOf(await post(image(10, { kind: "video" })))).toMatchObject({ status: 415, code: "unsupported_type" });
    expect(await errorOf(await post(image(10 * MiB + 1, { days: 2 })))).toMatchObject({ status: 413, code: "too_large" });
    expect((await post(image(20 * MiB, { mime: "image/gif" }))).status).toBe(201);
    expect(await errorOf(await post(image(20 * MiB + 1, { mime: "image/gif" })))).toMatchObject({ status: 413, code: "too_large" });
    expect(await errorOf(await post(video(200 * MiB + 1, 10)))).toMatchObject({ status: 413, code: "too_large" });
    expect(await errorOf(await post(video(10, 409601)))).toMatchObject({ status: 413, code: "too_large" });
    expect(await errorOf(await post(image(10, { days: 2 })))).toMatchObject({ status: 400, code: "bad_request" });
    expect(await errorOf(await post(image(10, { duration_ms: 5 })))).toMatchObject({ status: 400, code: "bad_request" });
    expect(await errorOf(await post(video(10, null as unknown as number)))).toMatchObject({ status: 400, code: "bad_request" });
    expect(await errorOf(await post(image(10, { title: "x".repeat(121) })))).toMatchObject({ status: 400, code: "bad_request" });
    expect(await errorOf(await post(image(0)))).toMatchObject({ status: 400, code: "bad_request" });
    expect((await post(image(10, { title: "é".repeat(120) }))).status).toBe(201);
  });
});

describe("parts", () => {
  it("refuses a part of the wrong length with conflict and stores nothing", async () => {
    const s = await createOk(await installToken(), image(1000));
    expect(await errorOf(await putPart(s, 1, bytes(999, PNG)))).toMatchObject({ status: 409, code: "conflict" });
    expect(await errorOf(await putPart(s, 1, bytes(1001, PNG)))).toMatchObject({ status: 409, code: "conflict" });
    expect(await errorOf(await call("POST", `/v1/shares/${s.id}/complete`, { token: s.upload_token }))).toMatchObject({ status: 409, code: "conflict" });
  });

  it("sizes every part at part_bytes except the last", async () => {
    const s = await createOk(await installToken(), video(PART + 7, 3));
    expect(s.parts).toBe(2);
    expect(await errorOf(await putPart(s, 1, bytes(7, MP4)))).toMatchObject({ status: 409, code: "conflict" });
    expect(await errorOf(await putPart(s, 2, bytes(PART)))).toMatchObject({ status: 409, code: "conflict" });
    expect((await putPart(s, 2, bytes(7))).status).toBe(200);
    expect(await errorOf(await putPart(s, 3, bytes(7)))).toMatchObject({ status: 400, code: "bad_request" });
  });

  it("will not complete while a part or the poster is missing", async () => {
    const s = await createOk(await installToken(), video(PART + 7, 3));
    const complete = () => call("POST", `/v1/shares/${s.id}/complete`, { token: s.upload_token });
    expect((await putPart(s, 2, bytes(7))).status).toBe(200);
    expect((await call("PUT", `/v1/shares/${s.id}/poster`, { token: s.upload_token, body: bytes(3, JPEG), headers: { "Content-Type": "image/jpeg" } })).status).toBe(200);
    expect(await errorOf(await complete())).toMatchObject({ status: 409, code: "conflict" });
    expect((await putPart(s, 1, bytes(PART, MP4))).status).toBe(200);
    expect((await complete()).status).toBe(200);
    expect((await testEnv.BUCKET.head(mediaKey(s)))?.httpMetadata?.contentDisposition).toBe(`inline; filename="easyshot-${s.id}.mp4"`);
    const poster = await testEnv.BUCKET.head(mediaKey(s).replace("media.mp4", "poster.jpg"));
    expect(poster?.httpMetadata).toMatchObject({ contentType: "image/jpeg", cacheControl: "public, max-age=3600" });
    expect(await errorOf(await putPart(s, 1, bytes(PART, MP4)))).toMatchObject({ status: 409, code: "conflict" });
  });

  it("replaces a part sent again", async () => {
    const s = await createOk(await installToken(), image(16));
    expect((await putPart(s, 1, bytes(16))).status).toBe(200);
    expect((await putPart(s, 1, bytes(16, PNG))).status).toBe(200);
    expect((await call("POST", `/v1/shares/${s.id}/complete`, { token: s.upload_token })).status).toBe(200);
  });

  it("takes a poster only for video, as image/jpeg of poster_size bytes", async () => {
    const t = await installToken();
    const img = await createOk(t, image(10));
    const poster = (s: typeof img, body: Uint8Array, type = "image/jpeg") =>
      call("PUT", `/v1/shares/${s.id}/poster`, { token: s.upload_token, body, headers: { "Content-Type": type } });
    expect(await errorOf(await poster(img, bytes(3, JPEG)))).toMatchObject({ status: 400, code: "bad_request" });
    const v = await createOk(t, video(10, 4));
    expect(await errorOf(await poster(v, bytes(4, JPEG), "image/png"))).toMatchObject({ status: 415, code: "unsupported_type" });
    expect(await errorOf(await poster(v, bytes(5, JPEG)))).toMatchObject({ status: 409, code: "conflict" });
    expect((await poster(v, bytes(4, JPEG))).status).toBe(200);
  });
});

describe("complete", () => {
  it("deletes a share whose bytes are not its type and answers unsupported_type", async () => {
    const s = await createOk(await installToken(), image(64, { mime: "image/webp" }));
    expect((await putPart(s, 1, bytes(64, PNG))).status).toBe(200);
    expect(await errorOf(await call("POST", `/v1/shares/${s.id}/complete`, { token: s.upload_token }))).toMatchObject({ status: 415, code: "unsupported_type" });
    expect(await testEnv.BUCKET.head(mediaKey(s))).toBeNull();
    expect(await errorOf(await call("DELETE", `/v1/shares/${s.id}`, { token: s.delete_token }))).toMatchObject({ status: 404, code: "not_found" });
  });

  it("deletes a video whose poster is not a JPEG", async () => {
    const s = await createOk(await installToken(), video(16, 4));
    expect((await putPart(s, 1, bytes(16, MP4))).status).toBe(200);
    expect((await call("PUT", `/v1/shares/${s.id}/poster`, { token: s.upload_token, body: bytes(4, PNG), headers: { "Content-Type": "image/jpeg" } })).status).toBe(200);
    expect(await errorOf(await call("POST", `/v1/shares/${s.id}/complete`, { token: s.upload_token }))).toMatchObject({ status: 415, code: "unsupported_type" });
    expect(await testEnv.BUCKET.head(mediaKey(s))).toBeNull();
    expect(await testEnv.BUCKET.head(mediaKey(s).replace("media.mp4", "poster.jpg"))).toBeNull();
  });

  it("accepts each type's signature", async () => {
    const t = await installToken();
    const sigs: [string, number[]][] = [
      ["image/png", PNG],
      ["image/jpeg", JPEG],
      ["image/gif", [..."GIF89a"].map((c) => c.charCodeAt(0))],
      ["image/webp", [..."RIFF\0\0\0\0WEBP"].map((c) => c.charCodeAt(0))],
    ];
    for (const [mime, magic] of sigs) {
      const s = await createOk(t, image(32, { mime }));
      expect((await putPart(s, 1, bytes(32, magic))).status).toBe(200);
      expect((await call("POST", `/v1/shares/${s.id}/complete`, { token: s.upload_token })).status, mime).toBe(200);
    }
  });

  it("answers a repeated complete with the same links", async () => {
    const s = await createOk(await installToken(), image(16));
    await putPart(s, 1, bytes(16, PNG));
    const a = await (await call("POST", `/v1/shares/${s.id}/complete`, { token: s.upload_token })).json();
    const b = await call("POST", `/v1/shares/${s.id}/complete`, { token: s.upload_token });
    expect(b.status).toBe(200);
    expect(await b.json()).toEqual(a);
  });
});

describe("complete in progress", () => {
  it("refuses part, poster and complete requests, and the poster it checked is the one served", async () => {
    const s = await createOk(await installToken(), video(16, 4));
    const checked = Uint8Array.of(...JPEG.slice(0, 3), 0x01);
    const putPoster = (body: Uint8Array) =>
      call("PUT", `/v1/shares/${s.id}/poster`, { token: s.upload_token, body, headers: { "Content-Type": "image/jpeg" } });
    const complete = () => call("POST", `/v1/shares/${s.id}/complete`, { token: s.upload_token });
    expect((await putPart(s, 1, bytes(16, MP4))).status).toBe(200);
    expect((await putPoster(checked)).status).toBe(200);

    const stub = ledger(testEnv);
    expect((await runInDurableObject(stub, (l: Ledger) => l.beginComplete(s.id))).ok).toBe(true);
    expect(await errorOf(await putPoster(Uint8Array.of(...JPEG.slice(0, 3), 0x02)))).toMatchObject({ status: 409, code: "conflict" });
    expect(await errorOf(await putPart(s, 1, bytes(16, MP4)))).toMatchObject({ status: 409, code: "conflict" });
    expect(await errorOf(await complete())).toMatchObject({ status: 409, code: "conflict" });
    await runInDurableObject(stub, (l: Ledger) => l.reopen(s.id));

    expect((await complete()).status).toBe(200);
    const posterKey = mediaKey(s).replace("media.mp4", "poster.jpg");
    const served = async () => new Uint8Array(await ((await testEnv.BUCKET.get(posterKey)) as R2ObjectBody).arrayBuffer());
    expect(await served()).toEqual(checked);
    expect(await testEnv.BUCKET.head(mediaKey(s).replace("media.mp4", "poster.upload"))).toBeNull();
    expect(await errorOf(await putPoster(Uint8Array.of(...JPEG.slice(0, 3), 0x03)))).toMatchObject({ status: 409, code: "conflict" });
    expect(await served()).toEqual(checked);
  });
});

describe("delete", () => {
  it("mid-upload aborts the upload and gives the byte-days back", async () => {
    const t = await installToken();
    const before = await left(t);
    const s = await createOk(t, video(PART + 10, 100, { days: 7 }));
    const during = await left(t);
    expect(before.byte_days_left - during.byte_days_left).toBe((PART + 10 + 100) * 7);
    expect((await putPart(s, 1, bytes(PART, MP4))).status).toBe(200);
    const uploadId = (await runInDurableObject(ledger(testEnv), (l: Ledger) => l.share(s.id)))?.upload_id;
    expect(uploadId).toBeTruthy();

    expect((await call("DELETE", `/v1/shares/${s.id}`, { token: s.delete_token })).status).toBe(204);
    expect((await left(t)).byte_days_left).toBe(before.byte_days_left);
    expect(await errorOf(await putPart(s, 2, bytes(10)))).toMatchObject({ status: 404, code: "not_found" });
    await expect(testEnv.BUCKET.resumeMultipartUpload(mediaKey(s), uploadId ?? "").uploadPart(2, bytes(10))).rejects.toThrow();
    expect(await testEnv.BUCKET.head(mediaKey(s))).toBeNull();
  });

  it("after complete removes the files but keeps the byte-days spent", async () => {
    const t = await installToken();
    const before = await left(t);
    const s = await createOk(t, image(16, { days: 3 }));
    await putPart(s, 1, bytes(16, PNG));
    expect((await call("POST", `/v1/shares/${s.id}/complete`, { token: s.upload_token })).status).toBe(200);
    expect(await testEnv.BUCKET.head(mediaKey(s))).not.toBeNull();
    expect((await call("DELETE", `/v1/shares/${s.id}`, { token: s.delete_token })).status).toBe(204);
    expect(await testEnv.BUCKET.head(mediaKey(s))).toBeNull();
    expect(before.byte_days_left - (await left(t)).byte_days_left).toBe(16 * 3);
  });
});
