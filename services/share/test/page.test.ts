import { runInDurableObject } from "cloudflare:test";
import { describe, expect, it } from "vitest";
import viewer from "../contract/viewer.json";
import template from "../viewer/viewer.html";
import { ledger, type Env } from "../src/env";
import { fill, FillFailure } from "../src/fill";
import type { Ledger } from "../src/ledger";
import { sharePage } from "../src/page";
import { bytes, call, createOk, image, installToken, JPEG, MP4, PNG, putPart, testEnv, video } from "./helpers";

const page = (id: string) => call("GET", `/${id}`);

async function liveImage(extra: Record<string, unknown> = {}) {
  const s = await createOk(await installToken(), image(16, extra));
  await putPart(s, 1, bytes(16, PNG));
  await call("POST", `/v1/shares/${s.id}/complete`, { token: s.upload_token });
  return s;
}

async function liveVideo() {
  const s = await createOk(await installToken(), video(16, 4, { title: "Demo", days: 3 }));
  await putPart(s, 1, bytes(16, MP4));
  await call("PUT", `/v1/shares/${s.id}/poster`, { token: s.upload_token, body: Uint8Array.of(...JPEG.slice(0, 3), 1), headers: { "Content-Type": "image/jpeg" } });
  await call("POST", `/v1/shares/${s.id}/complete`, { token: s.upload_token });
  return s;
}

function expire(id: string) {
  return runInDurableObject(ledger(testEnv), (l: Ledger) => {
    (l as unknown as { sql: SqlStorage }).sql.exec("UPDATE shares SET expires = ? WHERE id = ?", Date.now() - 1000, id);
  });
}

const GONE = "This share is gone";
const UPLOADING = "Still uploading";

function expectHeaders(r: Response) {
  expect(r.headers.get("Content-Type")).toBe("text/html; charset=utf-8");
  expect(r.headers.get("X-Robots-Tag")).toBe("noindex, nofollow");
  expect(r.headers.get("Referrer-Policy")).toBe("no-referrer");
  expect(r.headers.get("X-Content-Type-Options")).toBe("nosniff");
  expect(r.headers.get("X-Frame-Options")).toBe("DENY");
  expect(r.headers.get("Content-Security-Policy")).toBeTruthy();
}

describe("fill", () => {
  for (const c of viewer.cases) {
    it(c.name, () => {
      const run = () => fill(c.template, new Set(c.blocks), c.values as Record<string, string>);
      if ("error" in c) {
        expect(run).toThrow(FillFailure);
        try {
          run();
        } catch (e) {
          expect((e as FillFailure).code).toBe(c.error);
        }
      } else {
        expect(run()).toBe(c.output);
      }
    });
  }
});

describe("share page", () => {
  it("shows a ready image with its expiry and links to the media origin", async () => {
    const s = await liveImage({ title: "Login bug", width: 640, height: 480 });
    const r = await page(s.id);
    expect(r.status).toBe(200);
    expectHeaders(r);
    expect(r.headers.get("Cache-Control")).toBe("public, max-age=60");
    const h = await r.text();
    expect(h).toContain("<title>Login bug</title>");
    expect(h).toContain(`<img class="media" data-fit src="${s.media_url}" width="640" height="480"`);
    expect(h).toContain(`download="easyshot-${s.id}.png"`);
    expect(h).toContain(`<meta property="og:image" content="${s.media_url}">`);
    expect(h).toContain(`data-url="https://share.easyshot.site/${s.id}"`);
    expect(h).toMatch(/data-expires="\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}Z"/);
    expect(h).not.toContain("<video");
    expect(h).not.toContain(UPLOADING);
    expect(h).not.toContain(GONE);
    expect(h).not.toMatch(/<!--(if|end):|\{\{/);
    expect(r.headers.get("Content-Security-Policy")).toContain(`img-src data: ${testEnv.MEDIA_ORIGIN}`);
    expect(r.headers.get("Content-Security-Policy")).toContain(`media-src ${testEnv.MEDIA_ORIGIN}`);
  });

  it("shows a ready video with its poster", async () => {
    const s = await liveVideo();
    const r = await page(s.id);
    expect(r.status).toBe(200);
    const h = await r.text();
    expect(h).toContain(`<video class="media" data-fit src="${s.media_url}" poster="${s.poster_url}"`);
    expect(h).toContain(`<meta property="og:video:type" content="video/mp4">`);
    expect(h).toContain(`download="easyshot-${s.id}.mp4"`);
    expect(h).not.toContain('<img class="media"');
  });

  it("uses the default title when the share has none", async () => {
    const s = await liveImage();
    expect(await (await page(s.id)).text()).toContain("<title>Shared with easyshot</title>");
  });

  it("answers an unfinished upload with the uploading page, never cached", async () => {
    const s = await createOk(await installToken(), image(16));
    const r = await page(s.id);
    expect(r.status).toBe(200);
    expectHeaders(r);
    expect(r.headers.get("Cache-Control")).toBe("no-store");
    const h = await r.text();
    expect(h).toContain(UPLOADING);
    expect(h).not.toContain(GONE);
    expect(h).not.toContain('class="media"');
  });

  it("answers a share that is completing like one that is pending", async () => {
    const s = await createOk(await installToken(), image(16));
    await putPart(s, 1, bytes(16, PNG));
    await runInDurableObject(ledger(testEnv), (l: Ledger) => l.beginComplete(s.id));
    const r = await page(s.id);
    expect(r.status).toBe(200);
    expect(await r.text()).toContain(UPLOADING);
  });

  it("answers 410 once the share has expired", async () => {
    const s = await liveImage();
    await expire(s.id);
    const r = await page(s.id);
    expect(r.status).toBe(410);
    expectHeaders(r);
    const h = await r.text();
    expect(h).toContain(GONE);
    expect(h).not.toContain('class="media"');
  });

  it("answers 404 with the gone page for an unknown id, a deleted share and ids of the wrong shape", async () => {
    const s = await liveImage();
    expect((await call("DELETE", `/v1/shares/${s.id}`, { token: s.delete_token })).status).toBe(204);
    for (const id of [s.id, "7" + "a".repeat(21), "x".repeat(22), "7short", "7" + "a".repeat(22), "favicon.ico"]) {
      const r = await page(id);
      expect(r.status, id).toBe(404);
      expectHeaders(r);
      expect(await r.text()).toContain(GONE);
    }
  });

  it("never asks the Ledger about an id of the wrong shape", async () => {
    let asked = 0;
    const env = {
      ...testEnv,
      LEDGER: {
        idFromName: () => {
          asked++;
          throw new Error("ledger reached");
        },
      },
    } as unknown as Env;
    for (const id of ["x".repeat(22), "7short", "../v1/limits", "7" + "a".repeat(22)]) {
      expect((await sharePage(env, id)).status).toBe(404);
    }
    expect(asked).toBe(0);
  });

  it("escapes a title that carries markup", async () => {
    const s = await liveImage({ title: `<script>alert("x")</script>` });
    const h = await (await page(s.id)).text();
    expect(h).not.toContain(`<script>alert`);
    expect(h).toContain("&lt;script&gt;alert(&quot;x&quot;)&lt;/script&gt;");
  });

  it("allows exactly the inline script and style it serves, by hash", async () => {
    const s = await liveImage();
    const r = await page(s.id);
    const csp = r.headers.get("Content-Security-Policy") ?? "";
    const h = await r.text();
    const inline = [...h.matchAll(/<(script|style)>([\s\S]*?)<\/\1>/g)];
    expect(inline.map((m) => m[1]).sort()).toEqual(["script", "style"]);
    for (const [, tag, body] of inline) {
      const d = new Uint8Array(await crypto.subtle.digest("SHA-256", new TextEncoder().encode(body)));
      const hash = `'sha256-${btoa(String.fromCharCode(...d))}'`;
      expect(csp, tag).toContain(`${tag}-src ${hash}`);
    }
    expect(csp).not.toMatch(/script-src[^;]*unsafe-inline/);
    expect(csp).not.toMatch(/(^|; )style-src [^;]*unsafe-inline/);
    expect(template).toContain("<script>");
  });
});
