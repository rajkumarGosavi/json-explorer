// Minimal JSON syntax highlighter: turns a JSON string into HTML with a span
// per token. No dependencies. Output is HTML-escaped, so it's safe to render
// via v-html. Object keys (strings immediately followed by `:`) get their own
// class, distinct from string values.

const ESC: Record<string, string> = { "&": "&amp;", "<": "&lt;", ">": "&gt;" };

function esc(s: string): string {
  return s.replace(/[&<>]/g, (c) => ESC[c]);
}

export function highlightJson(input: string): string {
  let out = "";
  let i = 0;
  const n = input.length;
  while (i < n) {
    const ch = input[i];
    if (ch === '"') {
      // Consume a string, honouring backslash escapes.
      let j = i + 1;
      while (j < n) {
        if (input[j] === "\\") {
          j += 2;
          continue;
        }
        if (input[j] === '"') {
          j++;
          break;
        }
        j++;
      }
      const token = input.slice(i, j);
      // A string is a key if the next non-whitespace character is ':'.
      let k = j;
      while (k < n && /\s/.test(input[k])) k++;
      const cls = input[k] === ":" ? "json-key" : "json-string";
      out += `<span class="${cls}">${esc(token)}</span>`;
      i = j;
    } else if (ch === "-" || (ch >= "0" && ch <= "9")) {
      let j = i + 1;
      while (j < n && /[0-9.eE+-]/.test(input[j])) j++;
      out += `<span class="json-number">${esc(input.slice(i, j))}</span>`;
      i = j;
    } else if (input.startsWith("true", i) || input.startsWith("false", i)) {
      const lit = input[i] === "t" ? "true" : "false";
      out += `<span class="json-boolean">${lit}</span>`;
      i += lit.length;
    } else if (input.startsWith("null", i)) {
      out += `<span class="json-null">null</span>`;
      i += 4;
    } else if ("{}[],:".includes(ch)) {
      out += `<span class="json-punct">${ch}</span>`;
      i++;
    } else {
      out += esc(ch); // whitespace and anything else, escaped
      i++;
    }
  }
  return out;
}
