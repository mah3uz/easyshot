import { CHALLENGE_TTL_S, DAYS, DEFAULT_DAYS, type Env, ledger, limits } from "./env";
import { bearer, fail, json, readJson, rfc3339, secondsToMidnight, utcDay, versionOk } from "./http";
import { powOk } from "./pow";
import { hashIp, mintChallenge, mintInstall, openChallenge, openInstall } from "./tokens";

const tooOld = () => fail("version_unsupported", "app version below the minimum");

export async function challenge(req: Request, env: Env): Promise<Response> {
  if (!versionOk(req, env.MIN_APP_VERSION)) return tooOld();
  const { difficulty } = limits(env);
  const expS = Math.floor(Date.now() / 1000) + CHALLENGE_TTL_S;
  return json({ challenge: await mintChallenge(env.CHALLENGE_KEY, expS, difficulty), difficulty, expires_at: rfc3339(expS * 1000) });
}

export async function install(req: Request, env: Env): Promise<Response> {
  if (!versionOk(req, env.MIN_APP_VERSION)) return tooOld();
  const body = (await readJson(req)) as { challenge?: unknown; nonce?: unknown } | undefined;
  if (!body || typeof body.challenge !== "string" || typeof body.nonce !== "string") return fail("bad_request", "expected challenge and nonce");
  const now = Date.now();
  const c = await openChallenge(env.CHALLENGE_KEY, body.challenge);
  if (!c || c.expS * 1000 <= now) return fail("challenge_failed", "challenge not valid or expired");
  if (!(await powOk(body.challenge, body.nonce, c.difficulty))) return fail("challenge_failed", "proof of work does not meet the difficulty");
  const day = utcDay(now);
  const ip = await hashIp(env.IP_KEY, day, req.headers.get("CF-Connecting-IP") ?? "");
  const r = await ledger(env).install(c.id, c.expS * 1000, ip, now, day, limits(env).installsPerIpDay);
  if (r === "used") return fail("challenge_failed", "challenge already used");
  if (r === "ip_limit") return fail("rate_limited", "too many installs from this address today", secondsToMidnight(now));
  return json({ install_token: await mintInstall(env.INSTALL_KEY) });
}

export async function getLimits(req: Request, env: Env): Promise<Response> {
  if (!versionOk(req, env.MIN_APP_VERSION)) return tooOld();
  const l = limits(env);
  const token = bearer(req);
  const install = token ? await openInstall(env.INSTALL_KEY, token) : null;
  const now = Date.now();
  const u = await ledger(env).usage(install, now, utcDay(now));
  const body: Record<string, unknown> = {
    image_max_bytes: l.imageMax,
    gif_max_bytes: l.gifMax,
    video_max_bytes: l.videoMax,
    poster_max_bytes: l.posterMax,
    days: [...DAYS],
    default_days: DEFAULT_DAYS,
    part_bytes: l.partBytes,
    daily_shares: l.dailyShares,
    daily_byte_days: l.dailyByteDays,
  };
  if (install) {
    body.shares_left = Math.max(0, l.dailyShares - u.shares);
    body.byte_days_left = Math.max(0, l.dailyByteDays - u.byteDays);
  }
  body.accepting = u.live < l.storageStop;
  return json(body);
}
