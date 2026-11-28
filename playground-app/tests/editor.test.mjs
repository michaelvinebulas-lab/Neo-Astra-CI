import assert from "node:assert/strict";
import test from "node:test";

import { highlightSource, lineNumbers } from "../dist/editor.js";

test("highlightSource marks keywords, strings, and comments", () => {
  const html = highlightSource('pub fn main() { println("hello") // note\n}');
  assert.match(html, /tok-keyword/);
  assert.match(html, /tok-string/);
  assert.match(html, /tok-comment/);
});

test("highlightSource escapes HTML", () => {
  const html = highlightSource('let x = "<script>"');
  assert.equal(html.includes("<script>"), false);
  assert.match(html, /&lt;script&gt;/);
});

test("lineNumbers counts lines", () => {
  assert.equal(lineNumbers(""), "1");
  assert.equal(lineNumbers("a\nb\nc"), "1\n2\n3");
});
