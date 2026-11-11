import type { Ledger } from "./ledger";

export interface Env {
  BUCKET: R2Bucket;
  LEDGER: DurableObjectNamespace<Ledger>;
  BURST: RateLimit;
  IMAGE_MAX_BYTES: string | number;
  GIF_MAX_BYTES: string | number;
  VIDEO_MAX_BYTES: string | number;
  POSTER_MAX_BYTES: string | number;
  PART_BYTES: string | number;
  DAILY_SHARES: string | number;
  DAILY_BYTE_DAYS: string | number;
  STORAGE_STOP_BYTES: string | number;
  INSTALLS_PER_IP_DAY: string | number;
  POW_DIFFICULTY: string | number;
  MIN_APP_VERSION: string;
  MEDIA_ORIGIN: string;
  PAGE_ORIGIN: string;
  CHALLENGE_KEY: string;
  INSTALL_KEY: string;
  UPLOAD_KEY: string;
  DELETE_KEY: string;
  IP_KEY: string;
}

export const DAYS = [1, 3, 7] as const;
export const DEFAULT_DAYS = 7;
export const CHALLENGE_TTL_S = 300;
export const UPLOAD_TTL_S = 6 * 3600;
export const BURST_PERIOD_S = 60;

export interface Limits {
  imageMax: number;
  gifMax: number;
  videoMax: number;
  posterMax: number;
  partBytes: number;
  dailyShares: number;
  dailyByteDays: number;
  storageStop: number;
  installsPerIpDay: number;
  difficulty: number;
}

function count(name: string, v: string | number): number {
  const n = typeof v === "number" ? v : Number(v);
  if (!Number.isSafeInteger(n) || n <= 0) throw new Error(`${name} must be a positive integer`);
  return n;
}

export function limits(env: Env): Limits {
  const difficulty = count("POW_DIFFICULTY", env.POW_DIFFICULTY);
  if (difficulty > 255) throw new Error("POW_DIFFICULTY must fit in a byte");
  return {
    imageMax: count("IMAGE_MAX_BYTES", env.IMAGE_MAX_BYTES),
    gifMax: count("GIF_MAX_BYTES", env.GIF_MAX_BYTES),
    videoMax: count("VIDEO_MAX_BYTES", env.VIDEO_MAX_BYTES),
    posterMax: count("POSTER_MAX_BYTES", env.POSTER_MAX_BYTES),
    partBytes: count("PART_BYTES", env.PART_BYTES),
    dailyShares: count("DAILY_SHARES", env.DAILY_SHARES),
    dailyByteDays: count("DAILY_BYTE_DAYS", env.DAILY_BYTE_DAYS),
    storageStop: count("STORAGE_STOP_BYTES", env.STORAGE_STOP_BYTES),
    installsPerIpDay: count("INSTALLS_PER_IP_DAY", env.INSTALLS_PER_IP_DAY),
    difficulty,
  };
}

export function ledger(env: Env): DurableObjectStub<Ledger> {
  return env.LEDGER.get(env.LEDGER.idFromName("ledger"));
}
