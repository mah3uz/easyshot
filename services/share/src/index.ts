import type { Env } from "./env";
import { fail } from "./http";
import { challenge, getLimits, install } from "./install";
import { sharePage } from "./page";
import { complete, create, putPart, putPoster, remove, SHARE_ID } from "./shares";

export { Ledger } from "./ledger";

const SHARE_PATH = /^\/v1\/shares\/([^/]+)(?:\/(complete|poster|parts\/([1-9][0-9]{0,4})))?$/;
const HOME = "https://easyshot.site";

async function route(req: Request, env: Env): Promise<Response> {
  const path = new URL(req.url).pathname;
  const m = req.method;
  if (path === "/" && (m === "GET" || m === "HEAD")) return Response.redirect(HOME, 302);
  if (path === "/v1/challenge" && m === "GET") return challenge(req, env);
  if (path === "/v1/install" && m === "POST") return install(req, env);
  if (path === "/v1/limits" && m === "GET") return getLimits(req, env);
  if (path === "/v1/shares" && m === "POST") return create(req, env);
  const page = /^\/([^/]+)$/.exec(path);
  if (page && m === "GET") return sharePage(env, page[1]);
  const s = SHARE_PATH.exec(path);
  if (s && SHARE_ID.test(s[1])) {
    const [, id, action, n] = s;
    if (!action && m === "DELETE") return remove(req, env, id);
    if (action === "complete" && m === "POST") return complete(req, env, id);
    if (action === "poster" && m === "PUT") return putPoster(req, env, id);
    if (n && m === "PUT") return putPart(req, env, id, Number(n));
  }
  return fail("not_found", "no such resource");
}

export default {
  async fetch(req, env): Promise<Response> {
    if (req.headers.has("Origin")) return fail("origin_refused", "browser requests are not accepted");
    return route(req, env);
  },
} satisfies ExportedHandler<Env>;
