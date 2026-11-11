export type FillError = "missing_value" | "not_https";

export class FillFailure extends Error {
  constructor(readonly code: FillError, name: string) {
    super(`${code}: ${name}`);
  }
}

const BLOCK = /<!--if:([a-z]+)-->([\s\S]*?)<!--end:\1-->/;
const BLOCK_ALL = new RegExp(BLOCK.source, "g");
const PLACEHOLDER = /\{\{([a-z_]+)\}\}/g;

const ESCAPES: Record<string, string> = { "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" };

export function escapeHtml(s: string): string {
  return s.replace(/[&<>"']/g, (c) => ESCAPES[c]);
}

export function fill(template: string, blocks: ReadonlySet<string>, values: Readonly<Record<string, string>>): string {
  let out = template;
  while (BLOCK.test(out)) out = out.replace(BLOCK_ALL, (_, name: string, body: string) => (blocks.has(name) ? body : ""));
  return out.replace(PLACEHOLDER, (_, name: string) => {
    const v = Object.hasOwn(values, name) ? values[name] : undefined;
    if (v === undefined) throw new FillFailure("missing_value", name);
    if (name.endsWith("_url") && !v.startsWith("https://")) throw new FillFailure("not_https", name);
    return escapeHtml(v);
  });
}
