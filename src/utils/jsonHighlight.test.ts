import { describe, expect, it } from "vitest";
import { highlightJson } from "@/utils/jsonHighlight";

describe("highlightJson", () => {
  it("distinguishes object keys from string values", () => {
    const html = highlightJson('{"a": "b"}');
    expect(html).toContain('<span class="json-key">"a"</span>');
    expect(html).toContain('<span class="json-string">"b"</span>');
    expect(html).toContain('<span class="json-punct">{</span>');
  });

  it("classifies numbers, booleans, and null", () => {
    expect(highlightJson("123")).toContain('<span class="json-number">123</span>');
    expect(highlightJson("-4.5e2")).toContain('<span class="json-number">-4.5e2</span>');
    expect(highlightJson("true")).toContain('<span class="json-boolean">true</span>');
    expect(highlightJson("null")).toContain('<span class="json-null">null</span>');
  });

  it("HTML-escapes content inside strings and keys", () => {
    const html = highlightJson('{"a<b": "x&y>z"}');
    expect(html).toContain('<span class="json-key">"a&lt;b"</span>');
    expect(html).toContain('"x&amp;y&gt;z"');
    // no raw angle brackets leaked from the payload
    expect(html).not.toContain("a<b");
    expect(html).not.toContain("y>z");
  });
});
