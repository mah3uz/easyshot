export type Kind = "image" | "video";

export const MIMES: Record<string, { kind: Kind; ext: string }> = {
  "image/png": { kind: "image", ext: "png" },
  "image/jpeg": { kind: "image", ext: "jpg" },
  "image/webp": { kind: "image", ext: "webp" },
  "image/gif": { kind: "image", ext: "gif" },
  "video/mp4": { kind: "video", ext: "mp4" },
};

export const MAGIC_LEN = 12;

function startsWith(b: Uint8Array, at: number, sig: number[]): boolean {
  return b.length >= at + sig.length && sig.every((v, i) => b[at + i] === v);
}

const ascii = (s: string) => [...s].map((c) => c.charCodeAt(0));

export function magicOk(mime: string, b: Uint8Array): boolean {
  switch (mime) {
    case "image/png":
      return startsWith(b, 0, [0x89, 0x50, 0x4e, 0x47]);
    case "image/jpeg":
      return startsWith(b, 0, [0xff, 0xd8, 0xff]);
    case "image/gif":
      return startsWith(b, 0, ascii("GIF8"));
    case "image/webp":
      return startsWith(b, 0, ascii("RIFF")) && startsWith(b, 8, ascii("WEBP"));
    case "video/mp4":
      return startsWith(b, 4, ascii("ftyp"));
    default:
      return false;
  }
}

export function shareDir(id: string, days: number): string {
  return `s/${days}d/${id}`;
}

export function mediaKey(id: string, days: number, mime: string): string {
  return `${shareDir(id, days)}/media.${MIMES[mime].ext}`;
}

export function posterKey(id: string, days: number): string {
  return `${shareDir(id, days)}/poster.jpg`;
}

export function posterUploadKey(id: string, days: number): string {
  return `${shareDir(id, days)}/poster.upload`;
}

const CACHE = "public, max-age=3600";

export function mediaHttp(id: string, mime: string): R2HTTPMetadata {
  return { contentType: mime, cacheControl: CACHE, contentDisposition: `inline; filename="easyshot-${id}.${MIMES[mime].ext}"` };
}

export const POSTER_HTTP: R2HTTPMetadata = { contentType: "image/jpeg", cacheControl: CACHE, contentDisposition: "inline" };
