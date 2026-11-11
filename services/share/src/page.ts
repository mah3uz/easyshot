import template from "../viewer/viewer.html";
import { type Env, ledger } from "./env";
import { fill } from "./fill";
import { rfc3339 } from "./http";
import { MIMES } from "./media";
import { SHARE_ID, urls } from "./shares";

const DEFAULT_TITLE = "Shared with easyshot";
const HOME_URL = "https://easyshot.site";

const INLINE = /<(script|style)>([\s\S]*?)<\/\1>/g;

async function sha256(text: string): Promise<string> {
  const d = new Uint8Array(await crypto.subtle.digest("SHA-256", new TextEncoder().encode(text)));
  return `'sha256-${btoa(String.fromCharCode(...d))}'`;
}

let hashes: Promise<{ script: string[]; style: string[] }> | undefined;

function inlineHashes() {
  hashes ??= (async () => {
    const out: { script: string[]; style: string[] } = { script: [], style: [] };
    for (const [, tag, body] of template.matchAll(INLINE)) out[tag as "script" | "style"].push(await sha256(body));
    return out;
  })();
  return hashes;
}

async function csp(env: Env): Promise<string> {
  const h = await inlineHashes();
  return [
    "default-src 'none'",
    `script-src ${h.script.join(" ")}`,
    `style-src ${h.style.join(" ")}`,
    "style-src-attr 'unsafe-inline'",
    `img-src data: ${env.MEDIA_ORIGIN}`,
    `media-src ${env.MEDIA_ORIGIN}`,
    "font-src data:",
    "base-uri 'none'",
    "form-action 'none'",
    "frame-ancestors 'none'",
  ].join("; ");
}

async function html(env: Env, status: number, blocks: string[], values: Record<string, string>, cache: string): Promise<Response> {
  return new Response(fill(template, new Set(blocks), { title: DEFAULT_TITLE, home_url: HOME_URL, ...values }), {
    status,
    headers: {
      "Content-Type": "text/html; charset=utf-8",
      "Cache-Control": cache,
      "Content-Security-Policy": await csp(env),
      "X-Robots-Tag": "noindex, nofollow",
      "Referrer-Policy": "no-referrer",
      "X-Content-Type-Options": "nosniff",
      "X-Frame-Options": "DENY",
    },
  });
}

export async function sharePage(env: Env, id: string): Promise<Response> {
  const gone = (status: number) => html(env, status, ["gone"], {}, "no-store");
  if (!SHARE_ID.test(id)) return gone(404);
  const s = await ledger(env).share(id);
  if (!s) return gone(404);
  if (s.expires <= Date.now()) return gone(410);
  
  const title: Record<string, string> = s.title ? { title: s.title } : {};
  if (s.state !== "live") return html(env, 200, ["uploading"], title, "no-store");
  const u = urls(env, s);
  return html(
    env,
    200,
    ["ready", s.kind, "expires"],
    {
      ...title,
      page_url: u.page_url,
      media_url: u.media_url,
      poster_url: u.poster_url ?? u.media_url,
      mime: s.mime,
      width: String(s.width),
      height: String(s.height),
      expires_at: rfc3339(s.expires),
      download_name: `easyshot-${id}.${MIMES[s.mime].ext}`,
    },
    "public, max-age=60",
  );
}
