import test from "node:test";
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";
import { join } from "node:path";
import { generateTypeScript } from "../packages/codegen-ts/dist/index.js";

const root = fileURLToPath(new URL("../..", import.meta.url));
const binary = join(
  root,
  "target",
  "debug",
  process.platform === "win32" ? "astra-compiler.exe" : "astra-compiler",
);

function compileSource(source, { mode = "core", integerPolicy = "wrapping" } = {}) {
  const request = JSON.stringify({
    jsonrpc: "2.0",
    id: "codegen-ts",
    method: "compile",
    params: {
      apiVersion: "1.0",
      source: { path: "app/main.ns", text: source },
      target: "ts",
      mode,
      integerPolicy,
    },
  });

  const result = spawnSync(binary, [], { input: request, encoding: "utf8" });
  assert.equal(result.status, 0, result.stderr);
  const response = JSON.parse(result.stdout);
  assert.deepEqual(response.error, undefined);
  return response.result;
}

test("TypeScript backend emits typed function declarations and exports", () => {
  const compiled = compileSource([
    "module app.main",
    "",
    "pub fn add(a: i32, b: i32) -> i32 {",
    "    return a + b",
    "}",
    "",
    "pub fn main() -> i32 {",
    "    return add(2, 3)",
    "}",
    "",
  ].join("\n"));

  const generated = generateTypeScript({
    moduleName: "app.main",
    ir: compiled.ir,
  });

  assert.match(generated.code, /\/\/ Target: TypeScript ESM\./);
  assert.match(generated.code, /function add\(a: number, b: number\): number \{/);
  assert.match(generated.code, /function main\(\): number \{/);
  assert.match(generated.code, /export \{ add \};/);
  assert.match(generated.code, /export \{ main \};/);
  // Strict TypeScript: no implicit-any annotations anywhere in the output.
  assert.doesNotMatch(generated.code, /:\s*any\b/);
});
