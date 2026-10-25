/**
 * Stress tests — TYPE-004: Multi-source compiler API (check_project),
 * TYPE-002: Single-source check, FMT-003: astra fmt via the boundary.
 */

import test from "node:test";
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { join } from "node:path";
import { fileURLToPath } from "node:url";

const root = fileURLToPath(new URL("../..", import.meta.url));

function binary() {
  return join(
    root,
    "target",
    "debug",
    process.platform === "win32" ? "astra-compiler.exe" : "astra-compiler",
  );
}

function rpc(method, params, id = "test") {
  const payload = JSON.stringify({ jsonrpc: "2.0", id, method, params });
  const result = spawnSync(binary(), [], { input: payload, encoding: "utf8" });
  assert.equal(result.status, 0, `compiler crashed: ${result.stderr}`);
  const response = JSON.parse(result.stdout);
  assert.equal(response.jsonrpc, "2.0");
  assert.equal(response.id, id);
  return response;
}

function check(source, mode = "core") {
  return rpc("check", {
    apiVersion: "1.0",
    source: { path: "src/main.ns", text: source },
    target: "js",
    mode,
  });
}

function checkProject(sources, mode = "core") {
  return rpc("check_project", {
    apiVersion: "1.0",
    sources,
    target: "js",
    mode,
  });
}

// ─── initialize ────────────────────────────────────────────────────────────────

test("initialize lists check_project in commands", () => {
  const resp = rpc("initialize", undefined);
  assert.ok(resp.result.commands.includes("check_project"), "missing check_project");
  assert.ok(resp.result.features.includes("project-checker"), "missing project-checker feature");
});

// ─── Single-source check ──────────────────────────────────────────────────────

test("check passes a valid module", () => {
  const resp = check("pub fn main() {}");
  assert.equal(resp.result.diagnostics.length, 0, JSON.stringify(resp.result.diagnostics));
});

test("check catches NS1001 invalid comment", () => {
  const resp = check("/* block comment */");
  assert.ok(resp.result.diagnostics.some((d) => d.code === "NS1001"));
});

test("check catches NS2001 parse error", () => {
  const resp = check("fn main( {");
  assert.ok(resp.result.diagnostics.some((d) => d.code === "NS2001"));
});

test("check catches NS4001 type mismatch", () => {
  const resp = check('fn f() -> i32 { return "not a number" }');
  assert.ok(resp.result.diagnostics.some((d) => d.code === "NS4001"));
  assert.equal(resp.result.program, null);
});

test("check catches NS4007 missing return", () => {
  const resp = check("fn f() -> i32 { let x = 1 }");
  assert.ok(resp.result.diagnostics.some((d) => d.code === "NS4007"));
});

test("check catches NS4010 non-exhaustive match", () => {
  const resp = check("fn f(x: Option<i32>) -> str { return match x { Some(n) => \"s\" } }");
  assert.ok(resp.result.diagnostics.some((d) => d.code === "NS4010"));
});

test("check catches NS4011 unhandled error", () => {
  const resp = check("struct E(str)\nfn g() -> i32 ! E { return 0 }\nfn f() -> i32 { return g() }");
  assert.ok(resp.result.diagnostics.some((d) => d.code === "NS4011"));
});

test("check catches NS5001 mode requirement violation", () => {
  const resp = check("@requires(system)\nfn f() {}", "core");
  assert.ok(resp.result.diagnostics.some((d) => d.code === "NS5001"));
});

test("check passes with correct mode", () => {
  const resp = check("@requires(system)\nfn f() {}", "system");
  assert.equal(resp.result.diagnostics.length, 0);
});

test("check catches NS3001 private export", () => {
  const resp = check("private fn f() {}\nexport { f }");
  assert.ok(resp.result.diagnostics.some((d) => d.code === "NS3001"));
});

test("check catches NS3011 duplicate declaration", () => {
  const resp = check("fn f() {}\nfn f() {}");
  assert.ok(resp.result.diagnostics.some((d) => d.code === "NS3011"));
});

// ─── Format command ───────────────────────────────────────────────────────────

test("format normalizes keyword aliases", () => {
  const resp = rpc("format", {
    apiVersion: "1.0",
    source: { path: "src/main.ns", text: "public function f() {}\n" },
    target: "js",
    mode: "core",
  });
  assert.ok(resp.result.source.includes("pub fn f()"), `got: ${resp.result.source}`);
  assert.ok(!resp.result.source.includes("public function"));
});

test("format is idempotent", () => {
  const source = "fn f(x: i32) -> i32 {\n    return x + 1\n}\n";
  const resp1 = rpc("format", { apiVersion: "1.0", source: { path: "a.ns", text: source }, target: "js", mode: "core" });
  const resp2 = rpc("format", { apiVersion: "1.0", source: { path: "a.ns", text: resp1.result.source }, target: "js", mode: "core" });
  assert.equal(resp1.result.source, resp2.result.source, "format should be idempotent");
});

test("format preserves comments", () => {
  const resp = rpc("format", {
    apiVersion: "1.0",
    source: { path: "a.ns", text: "// top\nfn f() {\n    // inside\n    let x = 1 // inline\n}\n" },
    target: "js",
    mode: "core",
  });
  assert.ok(resp.result.source.includes("// top"));
  assert.ok(resp.result.source.includes("// inside"));
  assert.ok(resp.result.source.includes("// inline"));
});

// ─── check_project: valid programs ────────────────────────────────────────────

test("check_project passes a single valid module", () => {
  const resp = checkProject([{ key: "main", path: "main.ns", text: "pub fn main() {}\n" }]);
  assert.equal(resp.result.ok, true, JSON.stringify(resp.result));
});

test("check_project passes a two-module correct program", () => {
  const resp = checkProject([
    { key: "lib", path: "lib.ns", text: "pub fn add(a: i32, b: i32) -> i32 { return a + b }\n" },
    { key: "main", path: "main.ns", text: 'import { add } from "lib"\npub fn main() -> i32 { return add(1, 2) }\n' },
  ]);
  assert.equal(resp.result.ok, true, JSON.stringify(resp.result.modules));
  assert.equal(resp.result.modules.length, 2);
  assert.ok(resp.result.modules.every((m) => m.ok));
});

test("check_project module results preserve request order", () => {
  const resp = checkProject([
    { key: "a", path: "a.ns", text: "pub fn fa() {}\n" },
    { key: "b", path: "b.ns", text: "pub fn fb() {}\n" },
    { key: "c", path: "c.ns", text: "pub fn fc() {}\n" },
  ]);
  assert.equal(resp.result.ok, true);
  assert.deepEqual(resp.result.modules.map((m) => m.key), ["a", "b", "c"]);
});

test("check_project metadata contains module order", () => {
  const resp = checkProject([
    { key: "x", path: "x.ns", text: "pub fn fx() {}\n" },
    { key: "y", path: "y.ns", text: "pub fn fy() {}\n" },
  ]);
  assert.deepEqual(resp.result.metadata.moduleOrder, ["x", "y"]);
});

test("check_project passes three-module chain", () => {
  const resp = checkProject([
    { key: "a", path: "a.ns", text: "pub fn fa() -> i32 { return 1 }\n" },
    { key: "b", path: "b.ns", text: 'import { fa } from "a"\npub fn fb() -> i32 { return fa() + 1 }\n' },
    { key: "c", path: "c.ns", text: 'import { fb } from "b"\npub fn main() -> i32 { return fb() }\n' },
  ]);
  assert.equal(resp.result.ok, true, JSON.stringify(resp.result));
  assert.equal(resp.result.modules.length, 3);
});

// ─── check_project: cross-module type errors ──────────────────────────────────

test("check_project catches cross-module NS4001 type mismatch", () => {
  const resp = checkProject([
    { key: "lib", path: "lib.ns", text: "pub fn greet(name: str) -> str { return name }\n" },
    { key: "main", path: "main.ns", text: 'import { greet } from "lib"\npub fn run() -> str { return greet(42) }\n' },
  ]);
  assert.equal(resp.result.ok, false);
  const all = resp.result.modules.flatMap((m) => m.diagnostics);
  assert.ok(all.some((d) => d.code === "NS4001"), `expected NS4001: ${JSON.stringify(all)}`);
});

test("check_project catches cross-module NS4004 argument count error", () => {
  const resp = checkProject([
    { key: "math", path: "math.ns", text: "pub fn double(n: i32) -> i32 { return n * 2 }\n" },
    { key: "main", path: "main.ns", text: 'import { double } from "math"\npub fn run() -> i32 { return double(1, 2, 3) }\n' },
  ]);
  assert.equal(resp.result.ok, false);
  const all = resp.result.modules.flatMap((m) => m.diagnostics);
  assert.ok(all.some((d) => d.code === "NS4004"), `expected NS4004: ${JSON.stringify(all)}`);
});

test("check_project reports parse error in first module and stops", () => {
  const resp = checkProject([
    { key: "broken", path: "broken.ns", text: "fn bad( {" },
    { key: "fine", path: "fine.ns", text: "pub fn f() {}\n" },
  ]);
  assert.equal(resp.result.ok, false);
  const all = resp.result.modules.flatMap((m) => m.diagnostics);
  assert.ok(all.some((d) => d.code === "NS2001"), `expected NS2001: ${JSON.stringify(all)}`);
});

test("check_project rejects empty sources with -32602", () => {
  const resp = rpc("check_project", {
    apiVersion: "1.0",
    sources: [],
    target: "js",
    mode: "core",
  });
  assert.ok(resp.error != null, "should have a JSON-RPC error");
  assert.equal(resp.error.code, -32602);
});

test("check_project reports path in diagnostics", () => {
  const resp = checkProject([
    { key: "lib", path: "src/lib.ns", text: "pub fn f(n: str) -> str { return n }\n" },
    { key: "main", path: "src/main.ns", text: 'import { f } from "lib"\npub fn run() { f(99) }\n' },
  ]);
  assert.equal(resp.result.ok, false);
  const mainModule = resp.result.modules.find((m) => m.key === "main");
  assert.ok(mainModule, "main module should be present");
  assert.equal(mainModule.path, "src/main.ns");
});

// ─── Error handling ────────────────────────────────────────────────────────────

test("malformed JSON returns -32700", () => {
  const result = spawnSync(binary(), [], { input: "not json", encoding: "utf8" });
  const resp = JSON.parse(result.stdout);
  assert.ok(resp.error.code === -32700);
});

test("wrong apiVersion returns -32602", () => {
  const resp = rpc("check", {
    apiVersion: "99.0",
    source: { path: "main.ns", text: "fn main() {}" },
    target: "js",
    mode: "core",
  });
  assert.ok(resp.error != null);
  assert.equal(resp.error.code, -32602);
});

test("unknown method returns -32601", () => {
  const resp = rpc("nonexistent_method", {});
  assert.ok(resp.error != null);
  assert.equal(resp.error.code, -32601);
});
