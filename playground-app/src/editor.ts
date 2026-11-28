/** Lightweight code-editor surface: highlighted layer, gutter, and tab handling. */

const KEYWORDS = new Set([
  "as", "break", "const", "continue", "default", "defer", "else", "enum",
  "export", "false", "fn", "for", "foreign", "from", "if", "import", "in",
  "let", "match", "module", "mut", "not", "panic", "abort", "private", "pub",
  "return", "spawn", "struct", "true", "type", "while", "fail",
]);

function escapeHtml(value: string): string {
  return value
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;")
    .replaceAll('"', "&quot;");
}

function span(kind: string, text: string): string {
  return `<span class="tok-${kind}">${escapeHtml(text)}</span>`;
}

export function highlightSource(text: string): string {
  let result = "";
  let i = 0;
  while (i < text.length) {
    const c = text[i] as string;
    if (c === "/" && text[i + 1] === "/") {
      let end = i;
      while (end < text.length && text[end] !== "\n") end++;
      result += span("comment", text.slice(i, end));
      i = end;
      continue;
    }
    if (c === '"') {
      let end = i + 1;
      while (end < text.length) {
        if (text[end] === "\\") {
          end += 2;
          continue;
        }
        if (text[end] === '"') {
          end++;
          break;
        }
        end++;
      }
      result += span("string", text.slice(i, end));
      i = end;
      continue;
    }
    if (/[0-9]/.test(c)) {
      let end = i;
      while (end < text.length && /[0-9a-fA-FxX._+-]/.test(text[end] as string)) end++;
      result += span("number", text.slice(i, end));
      i = end;
      continue;
    }
    if (/[A-Za-z_]/.test(c)) {
      let end = i;
      while (end < text.length && /[A-Za-z0-9_]/.test(text[end] as string)) end++;
      const word = text.slice(i, end);
      if (KEYWORDS.has(word)) result += span("keyword", word);
      else if (/^[A-Z]/.test(word)) result += span("type", word);
      else result += escapeHtml(word);
      i = end;
      continue;
    }
    result += escapeHtml(c);
    i++;
  }
  return result;
}

export function lineNumbers(text: string): string {
  const count = text === "" ? 1 : text.split("\n").length;
  return Array.from({ length: count }, (_, index) => String(index + 1)).join("\n");
}
