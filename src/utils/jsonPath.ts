// JSONPath (`$.a.b[3]`) formatting and parsing, shared by the inspector, the
// tree copy menu, and goto-path. `PathSegment.key` set = object entry (its
// `index` is the numeric child index, irrelevant when formatting/parsing);
// `key === null` = array element addressed by `index`.
import type { PathSegment } from "@/types/json";

const IDENT_RE = /^[A-Za-z_][A-Za-z0-9_]*$/;

/** Render an ancestor chain as a JSONPath string, e.g. `$.a.b[3]`. */
export function pathToString(segs: PathSegment[]): string {
  let out = "$";
  for (const s of segs) {
    if (s.key !== null) {
      out += IDENT_RE.test(s.key) ? `.${s.key}` : `[${JSON.stringify(s.key)}]`;
    } else {
      out += `[${s.index}]`;
    }
  }
  return out;
}

/**
 * Inverse of `pathToString`. Returns the segment list, or `null` on malformed
 * input. Object steps carry `{ key, index: 0 }` (the walk matches by key, not
 * index); array steps carry `{ key: null, index }`. Accepts both `["k"]` and
 * `['k']` quoting so hand-typed paths are forgiving.
 */
export function parseJsonPath(input: string): PathSegment[] | null {
  const s = input.trim();
  if (s[0] !== "$") return null;
  const out: PathSegment[] = [];
  let i = 1;
  while (i < s.length) {
    const ch = s[i];
    if (ch === ".") {
      i++;
      const start = i;
      while (i < s.length && /[A-Za-z0-9_]/.test(s[i])) i++;
      if (i === start) return null; // empty `.` accessor
      out.push({ key: s.slice(start, i), index: 0 });
    } else if (ch === "[") {
      i++;
      const q = s[i];
      if (q === '"' || q === "'") {
        i++;
        let raw = "";
        while (i < s.length && s[i] !== q) {
          if (s[i] === "\\") {
            raw += s[i] + (s[i + 1] ?? "");
            i += 2;
          } else {
            raw += s[i];
            i++;
          }
        }
        if (s[i] !== q) return null; // unterminated
        i++;
        if (s[i] !== "]") return null;
        i++;
        // Normalize to a double-quoted JSON string so JSON.parse handles escapes.
        const json =
          q === '"'
            ? `"${raw}"`
            : `"${raw.replace(/\\'/g, "'").replace(/"/g, '\\"')}"`;
        let key: string;
        try {
          key = JSON.parse(json);
        } catch {
          return null;
        }
        out.push({ key, index: 0 });
      } else {
        const start = i;
        while (i < s.length && /[0-9]/.test(s[i])) i++;
        if (i === start || s[i] !== "]") return null;
        out.push({ key: null, index: Number(s.slice(start, i)) });
        i++;
      }
    } else {
      return null;
    }
  }
  return out;
}
