import { describe, expect, it } from "vitest";
import type { PathSegment } from "@/types/json";
import { parseJsonPath, pathToString } from "@/utils/jsonPath";

const key = (k: string): PathSegment => ({ key: k, index: 0 });
const idx = (i: number): PathSegment => ({ key: null, index: i });

describe("pathToString", () => {
  it("renders the root as $", () => {
    expect(pathToString([])).toBe("$");
  });

  it("uses dot notation for identifier keys", () => {
    expect(pathToString([key("a"), key("b")])).toBe("$.a.b");
  });

  it("uses bracket-quote notation for non-identifier keys", () => {
    expect(pathToString([key("weird key")])).toBe('$["weird key"]');
    expect(pathToString([key("a-b")])).toBe('$["a-b"]');
    expect(pathToString([key("")])).toBe('$[""]');
  });

  it("uses bracket-index notation for array elements", () => {
    expect(pathToString([key("a"), idx(2), key("b")])).toBe("$.a[2].b");
  });
});

describe("parseJsonPath", () => {
  it("parses the root", () => {
    expect(parseJsonPath("$")).toEqual([]);
  });

  it("parses dot, index, and quoted-bracket accessors", () => {
    expect(parseJsonPath("$.a.b")).toEqual([key("a"), key("b")]);
    expect(parseJsonPath("$.a[3]")).toEqual([key("a"), idx(3)]);
    expect(parseJsonPath('$["weird key"].b')).toEqual([key("weird key"), key("b")]);
    expect(parseJsonPath("$[0]")).toEqual([idx(0)]);
  });

  it("accepts single-quoted keys as a convenience", () => {
    expect(parseJsonPath("$['k']")).toEqual([key("k")]);
  });

  it("rejects malformed input", () => {
    expect(parseJsonPath("a.b")).toBeNull(); // missing $
    expect(parseJsonPath("$.")).toBeNull(); // empty accessor
    expect(parseJsonPath("$[x]")).toBeNull(); // non-numeric, unquoted
    expect(parseJsonPath("$[1")).toBeNull(); // unterminated bracket
  });
});

describe("round trip", () => {
  it("pathToString ∘ parseJsonPath is identity on canonical paths", () => {
    for (const p of ["$", "$.a", "$.a.b[3]", '$["weird key"]', "$[0][1].c"]) {
      const segs = parseJsonPath(p);
      expect(segs).not.toBeNull();
      expect(pathToString(segs!)).toBe(p);
    }
  });
});
