import { DurableObject } from "cloudflare:workers";
import type { Env } from "./env";

const DAY_MS = 86_400_000;

export type State = "pending" | "completing" | "live";

export interface Share {
  id: string;
  install: string;
  day: string;
  kind: string;
  mime: string;
  size: number;
  poster_size: number;
  days: number;
  part_bytes: number;
  parts: number;
  title: string | null;
  width: number;
  height: number;
  duration_ms: number | null;
  created: number;
  expires: number;
  upload_id: string | null;
  state: State;
  poster: number;
}

export type NewShare = Omit<Share, "upload_id" | "state" | "poster">;

export interface Quota {
  dailyShares: number;
  dailyByteDays: number;
  storageStop: number;
}

export type Reserve = { ok: true } | { ok: false; error: "quota_daily" } | { ok: false; error: "full"; retryAfterS: number };

export type Begin =
  | { ok: true; share: Share; parts: { partNumber: number; etag: string }[] }
  | { ok: false; why: "gone" | "busy" | "missing" | "live"; share?: Share };

export function bytesOf(s: { size: number; poster_size: number }): number {
  return s.size + s.poster_size;
}

export function costOf(s: { size: number; poster_size: number; days: number }): number {
  return bytesOf(s) * s.days;
}

// Methods stay synchronous: with no await, each call runs whole, so check-then-write is atomic.
export class Ledger extends DurableObject<Env> {
  private sql: SqlStorage;

  constructor(ctx: DurableObjectState, env: Env) {
    super(ctx, env);
    this.sql = ctx.storage.sql;
    this.sql.exec(`
      CREATE TABLE IF NOT EXISTS used_challenges (id TEXT PRIMARY KEY, exp INTEGER NOT NULL);
      CREATE INDEX IF NOT EXISTS used_challenges_exp ON used_challenges (exp);
      CREATE TABLE IF NOT EXISTS ip_installs (ip TEXT NOT NULL, day TEXT NOT NULL, n INTEGER NOT NULL, PRIMARY KEY (ip, day));
      CREATE INDEX IF NOT EXISTS ip_installs_day ON ip_installs (day);
      CREATE TABLE IF NOT EXISTS install_days (install TEXT NOT NULL, day TEXT NOT NULL, shares INTEGER NOT NULL, byte_days INTEGER NOT NULL, PRIMARY KEY (install, day));
      CREATE INDEX IF NOT EXISTS install_days_day ON install_days (day);
      CREATE TABLE IF NOT EXISTS shares (
        id TEXT PRIMARY KEY, install TEXT NOT NULL, day TEXT NOT NULL, kind TEXT NOT NULL, mime TEXT NOT NULL,
        size INTEGER NOT NULL, poster_size INTEGER NOT NULL, days INTEGER NOT NULL, part_bytes INTEGER NOT NULL,
        parts INTEGER NOT NULL, title TEXT, width INTEGER NOT NULL, height INTEGER NOT NULL, duration_ms INTEGER,
        created INTEGER NOT NULL, expires INTEGER NOT NULL, hold_until INTEGER NOT NULL, upload_id TEXT,
        state TEXT NOT NULL, poster INTEGER NOT NULL DEFAULT 0);
      CREATE INDEX IF NOT EXISTS shares_hold ON shares (hold_until);
      CREATE TABLE IF NOT EXISTS parts (id TEXT NOT NULL, n INTEGER NOT NULL, etag TEXT NOT NULL, PRIMARY KEY (id, n));
      CREATE TABLE IF NOT EXISTS meta (k TEXT PRIMARY KEY, v INTEGER NOT NULL);
      INSERT OR IGNORE INTO meta (k, v) VALUES ('live_bytes', 0);
    `);
  }

  private prune(now: number, day: string): void {
    this.sql.exec("DELETE FROM used_challenges WHERE exp <= ?", now);
    this.sql.exec("DELETE FROM ip_installs WHERE day < ?", day);
    this.sql.exec("DELETE FROM install_days WHERE day < ?", day);
    const gone = this.sql.exec<{ b: number | null }>("SELECT SUM(size + poster_size) AS b FROM shares WHERE hold_until <= ?", now).one().b;
    if (gone) {
      this.sql.exec("DELETE FROM parts WHERE id IN (SELECT id FROM shares WHERE hold_until <= ?)", now);
      this.sql.exec("DELETE FROM shares WHERE hold_until <= ?", now);
      this.addLive(-gone);
    }
  }

  private live(): number {
    return this.sql.exec<{ v: number }>("SELECT v FROM meta WHERE k = 'live_bytes'").one().v;
  }

  private addLive(delta: number): void {
    this.sql.exec("UPDATE meta SET v = v + ? WHERE k = 'live_bytes'", delta);
  }

  private spent(install: string, day: string): { shares: number; byteDays: number } {
    const r = this.sql.exec<{ shares: number; byte_days: number }>("SELECT shares, byte_days FROM install_days WHERE install = ? AND day = ?", install, day).toArray()[0];
    return { shares: r?.shares ?? 0, byteDays: r?.byte_days ?? 0 };
  }

  private get(id: string): Share | null {
    return (this.sql.exec("SELECT * FROM shares WHERE id = ?", id).toArray()[0] as unknown as Share | undefined) ?? null;
  }

  install(challenge: string, challengeExpMs: number, ip: string, now: number, day: string, perIpDay: number): "ok" | "used" | "ip_limit" {
    this.prune(now, day);
    if (this.sql.exec("SELECT 1 FROM used_challenges WHERE id = ?", challenge).toArray().length > 0) return "used";
    const n = this.sql.exec<{ n: number }>("SELECT n FROM ip_installs WHERE ip = ? AND day = ?", ip, day).toArray()[0]?.n ?? 0;
    if (n >= perIpDay) return "ip_limit";
    // Kept a minute past expiry so a Worker whose clock lags cannot accept it after it is pruned.
    this.sql.exec("INSERT INTO used_challenges (id, exp) VALUES (?, ?)", challenge, challengeExpMs + 60_000);
    this.sql.exec("INSERT INTO ip_installs (ip, day, n) VALUES (?, ?, 1) ON CONFLICT (ip, day) DO UPDATE SET n = n + 1", ip, day);
    return "ok";
  }

  usage(install: string | null, now: number, day: string): { live: number; shares: number; byteDays: number } {
    this.prune(now, day);
    const s = install ? this.spent(install, day) : { shares: 0, byteDays: 0 };
    return { live: this.live(), ...s };
  }

  reserve(s: NewShare, q: Quota): Reserve {
    this.prune(s.created, s.day);
    const spent = this.spent(s.install, s.day);
    const cost = costOf(s);
    if (spent.shares + 1 > q.dailyShares || spent.byteDays + cost > q.dailyByteDays) return { ok: false, error: "quota_daily" };
    if (this.live() + bytesOf(s) > q.storageStop) {
      const next = this.sql.exec<{ h: number | null }>("SELECT MIN(hold_until) AS h FROM shares").one().h;
      return { ok: false, error: "full", retryAfterS: next ? (next - s.created) / 1000 : 3600 };
    }
    this.sql.exec(
      `INSERT INTO shares (id, install, day, kind, mime, size, poster_size, days, part_bytes, parts, title, width, height,
        duration_ms, created, expires, hold_until, state) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 'pending')`,
      s.id, s.install, s.day, s.kind, s.mime, s.size, s.poster_size, s.days, s.part_bytes, s.parts, s.title, s.width, s.height,
      s.duration_ms, s.created, s.expires, s.expires + DAY_MS,
    );
    this.sql.exec(
      "INSERT INTO install_days (install, day, shares, byte_days) VALUES (?, ?, 1, ?) ON CONFLICT (install, day) DO UPDATE SET shares = shares + 1, byte_days = byte_days + ?",
      s.install, s.day, cost, cost,
    );
    this.addLive(bytesOf(s));
    return { ok: true };
  }

  setUpload(id: string, uploadId: string): boolean {
    return this.sql.exec("UPDATE shares SET upload_id = ? WHERE id = ? AND upload_id IS NULL", uploadId, id).rowsWritten > 0;
  }

  share(id: string): Share | null {
    return this.get(id);
  }

  recordPart(id: string, n: number, etag: string): "ok" | "gone" | "closed" {
    const s = this.get(id);
    if (!s) return "gone";
    if (s.state !== "pending") return "closed";
    this.sql.exec("INSERT INTO parts (id, n, etag) VALUES (?, ?, ?) ON CONFLICT (id, n) DO UPDATE SET etag = excluded.etag", id, n, etag);
    return "ok";
  }

  recordPoster(id: string): "ok" | "gone" | "closed" {
    const s = this.get(id);
    if (!s) return "gone";
    if (s.state !== "pending") return "closed";
    this.sql.exec("UPDATE shares SET poster = 1 WHERE id = ?", id);
    return "ok";
  }

  beginComplete(id: string): Begin {
    const s = this.get(id);
    if (!s) return { ok: false, why: "gone" };
    if (s.state === "live") return { ok: false, why: "live", share: s };
    if (s.state === "completing") return { ok: false, why: "busy" };
    const parts = this.sql
      .exec<{ partNumber: number; etag: string }>("SELECT n AS partNumber, etag FROM parts WHERE id = ? ORDER BY n", id)
      .toArray();
    if (parts.length !== s.parts || (s.kind === "video" && !s.poster)) return { ok: false, why: "missing" };
    this.sql.exec("UPDATE shares SET state = 'completing' WHERE id = ?", id);
    return { ok: true, share: s, parts };
  }

  reopen(id: string): void {
    this.sql.exec("UPDATE shares SET state = 'pending' WHERE id = ? AND state = 'completing'", id);
  }

  finishComplete(id: string): boolean {
    return this.sql.exec("UPDATE shares SET state = 'live' WHERE id = ? AND state = 'completing'", id).rowsWritten > 0;
  }

  remove(id: string): Share | null {
    const s = this.get(id);
    if (!s) return null;
    if (s.state !== "live") {
      this.sql.exec("UPDATE install_days SET byte_days = MAX(0, byte_days - ?) WHERE install = ? AND day = ?", costOf(s), s.install, s.day);
    }
    this.sql.exec("DELETE FROM parts WHERE id = ?", id);
    this.sql.exec("DELETE FROM shares WHERE id = ?", id);
    this.addLive(-bytesOf(s));
    return s;
  }
}
