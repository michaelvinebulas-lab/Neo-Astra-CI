import test from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";

const root = fileURLToPath(new URL("../..", import.meta.url));
const request = JSON.stringify({
  jsonrpc: "2.0",
  id: "smoke",
  method: "check",
  params: {
    apiVersion: "1.0",
    source: { path: "src/main.ns", text: "" },
    target: "js",
    mode: "core",
  },
});

test("native compiler entrypoint speaks JSON-RPC 2.0", () => {
  const binary = join(
    root,
    "target",
    "debug",
    process.platform === "win32" ? "astra-compiler.exe" : "astra-compiler",
  );

  const result = spawnSync(binary, [], {
    input: request,
    encoding: "utf8",
  });

  assert.equal(result.status, 0, result.stderr);
  const response = JSON.parse(result.stdout);
  assert.equal(response.jsonrpc, "2.0");
  assert.equal(response.id, "smoke");
  assert.equal(response.result.apiVersion, "1.0");
  assert.deepEqual(response.result.program, { name: "" });
  assert.deepEqual(response.result.diagnostics, []);
});

test("native compiler reports type-checking diagnostics", () => {
  const binary = join(
    root,
    "target",
    "debug",
    process.platform === "win32" ? "astra-compiler.exe" : "astra-compiler",
  );
  const typeRequest = JSON.stringify({
    jsonrpc: "2.0",
    id: "types",
    method: "check",
    params: {
      apiVersion: "1.0",
      source: {
        path: "src/main.ns",
        text: 'fn wrong() -> i32 {\n    return "no"\n}\n',
      },
      target: "js",
      mode: "core",
    },
  });

  const result = spawnSync(binary, [], {
    input: typeRequest,
    encoding: "utf8",
  });

  assert.equal(result.status, 0, result.stderr);
  const response = JSON.parse(result.stdout);
  assert.equal(response.id, "types");
  assert.equal(response.result.program, null);
  assert.ok(
    response.result.diagnostics.some(
      (diagnostic) => diagnostic.code === "NS4001",
    ),
  );
});

test("native compiler infers a mode when mode is omitted", () => {
  const binary = join(
    root,
    "target",
    "debug",
    process.platform === "win32" ? "astra-compiler.exe" : "astra-compiler",
  );
  const inferredRequest = JSON.stringify({
    jsonrpc: "2.0",
    id: "inferred",
    method: "check",
    params: {
      apiVersion: "1.0",
      source: {
        path: "src/client.ns",
        text: '@tags(fetch)\nmodule app.client\npub fn load() {}\n',
      },
      target: "js",
    },
  });

  const result = spawnSync(binary, [], {
    input: inferredRequest,
    encoding: "utf8",
  });

  assert.equal(result.status, 0, result.stderr);
  const response = JSON.parse(result.stdout);
  assert.equal(response.id, "inferred");
  assert.deepEqual(response.result.diagnostics, []);
  assert.equal(response.result.metadata.mode, "web");
  assert.equal(response.result.metadata.modeSource, "inferred");
  assert.ok(
    response.result.metadata.modeReasons.some(
      (reason) => reason.capability === "fetch",
    ),
  );
});

test("native compiler formats source canonically", () => {
  const binary = join(
    root,
    "target",
    "debug",
    process.platform === "win32" ? "astra-compiler.exe" : "astra-compiler",
  );
  const formatRequest = JSON.stringify({
    jsonrpc: "2.0",
    id: "fmt",
    method: "format",
    params: {
      apiVersion: "1.0",
      source: {
        path: "src/main.ns",
        text: "// doc\npublic function f() {\n    // body\n    var mut x = 1 // value\n}\n",
      },
      target: "js",
      mode: "core",
    },
  });

  const result = spawnSync(binary, [], {
    input: formatRequest,
    encoding: "utf8",
  });

  assert.equal(result.status, 0, result.stderr);
  const response = JSON.parse(result.stdout);
  assert.equal(response.id, "fmt");
  assert.match(response.result.source, /pub fn f\(\)/);
  assert.match(response.result.source, /let mut x = 1/);
  assert.match(response.result.source, /\/\/ doc/);
  assert.match(response.result.source, /\/\/ body/);
  assert.match(response.result.source, /\/\/ value/);
});

test("Wasm entrypoint returns a compiler response through linear memory", async () => {
  const wasmPath = join(
    root,
    "target",
    "wasm32-unknown-unknown",
    "debug",
    "astra_wasm.wasm",
  );
  const module = await WebAssembly.compile(readFileSync(wasmPath));
  const instance = await WebAssembly.instantiate(module);
  const exports = instance.exports;

  assert.equal(exports.astra_protocol_version(), 1);

  const input = Buffer.from(request, "utf8");
  const inputPointer = exports.astra_alloc(input.length);
  new Uint8Array(exports.memory.buffer, inputPointer, input.length).set(input);

  const packed = BigInt(exports.astra_compile(inputPointer, input.length));
  const outputPointer = Number(packed >> 32n);
  const outputLength = Number(packed & 0xffff_ffffn);
  const output = Buffer.from(
    new Uint8Array(exports.memory.buffer, outputPointer, outputLength),
  ).toString("utf8");

  exports.astra_free(inputPointer, input.length);
  exports.astra_free(outputPointer, outputLength);

  const response = JSON.parse(output);
  assert.equal(response.jsonrpc, "2.0");
  assert.equal(response.id, "smoke");
  assert.equal(response.result.apiVersion, "1.0");
  assert.deepEqual(response.result.program, { name: "" });
  assert.deepEqual(response.result.diagnostics, []);
});

// ─── check_project integration tests ─────────────────────────────────────────

function projectRequest(id, sources, mode = "core") {
  return JSON.stringify({
    jsonrpc: "2.0",
    id,
    method: "check_project",
    params: {
      apiVersion: "1.0",
      sources,
      target: "js",
      ...(mode === null ? {} : { mode }),
    },
  });
}

test("check_project: valid two-module project passes", () => {
  const binary = join(
    root,
    "target",
    "debug",
    process.platform === "win32" ? "astra-compiler.exe" : "astra-compiler",
  );

  const req = projectRequest("proj-ok", [
    {
      key: "lib",
      path: "src/lib.ns",
      text: "pub fn add(a: i32, b: i32) -> i32 { return a + b }\n",
    },
    {
      key: "main",
      path: "src/main.ns",
      text: 'import { add } from "lib"\npub fn run() -> i32 { return add(1, 2) }\n',
    },
  ]);

  const result = spawnSync(binary, [], { input: req, encoding: "utf8" });
  assert.equal(result.status, 0, result.stderr);

  const response = JSON.parse(result.stdout);
  assert.equal(response.jsonrpc, "2.0");
  assert.equal(response.id, "proj-ok");
  assert.equal(response.result.ok, true, `expected ok=true: ${JSON.stringify(response.result)}`);
  assert.equal(response.result.apiVersion, "1.0");

  const keys = response.result.modules.map((m) => m.key);
  assert.ok(keys.includes("lib"), "lib should be in modules");
  assert.ok(keys.includes("main"), "main should be in modules");

  for (const mod of response.result.modules) {
    assert.equal(mod.ok, true, `module ${mod.key} should be ok`);
    assert.deepEqual(mod.diagnostics, [], `module ${mod.key} should have no diagnostics`);
  }
});

test("check_project: omitted mode is inferred across modules", () => {
  const binary = join(
    root,
    "target",
    "debug",
    process.platform === "win32" ? "astra-compiler.exe" : "astra-compiler",
  );

  const req = projectRequest(
    "proj-inferred",
    [
      {
        key: "core",
        path: "src/core.ns",
        text: "pub fn helper() {}\n",
      },
      {
        key: "web",
        path: "src/web.ns",
        text: "@tags(fetch)\npub fn load() {}\n",
      },
    ],
    null,
  );

  const result = spawnSync(binary, [], { input: req, encoding: "utf8" });
  assert.equal(result.status, 0, result.stderr);

  const response = JSON.parse(result.stdout);
  assert.equal(response.result.ok, true, JSON.stringify(response.result));
  assert.equal(response.result.metadata.mode, "web");
  assert.equal(response.result.metadata.modeSource, "inferred");
  assert.deepEqual(response.result.metadata.inferredModes, {
    core: "core",
    web: "web",
  });
});

test("check_project: cross-module type error is caught", () => {
  const binary = join(
    root,
    "target",
    "debug",
    process.platform === "win32" ? "astra-compiler.exe" : "astra-compiler",
  );

  const req = projectRequest("proj-err", [
    {
      key: "lib",
      path: "src/lib.ns",
      text: "pub fn greet(name: str) -> str { return name }\n",
    },
    {
      key: "main",
      path: "src/main.ns",
      // Passes i32 where str is expected
      text: 'import { greet } from "lib"\npub fn run() -> str { return greet(42) }\n',
    },
  ]);

  const result = spawnSync(binary, [], { input: req, encoding: "utf8" });
  assert.equal(result.status, 0, result.stderr);

  const response = JSON.parse(result.stdout);
  assert.equal(response.result.ok, false, "cross-module type error should fail");

  const allDiagnostics = response.result.modules.flatMap((m) => m.diagnostics);
  assert.ok(
    allDiagnostics.some((d) => d.code === "NS4001"),
    `expected NS4001 in diagnostics: ${JSON.stringify(allDiagnostics)}`,
  );
});

test("check_project: module order is preserved in response", () => {
  const binary = join(
    root,
    "target",
    "debug",
    process.platform === "win32" ? "astra-compiler.exe" : "astra-compiler",
  );

  const req = projectRequest("proj-order", [
    { key: "alpha", path: "alpha.ns", text: "pub fn a() {}\n" },
    { key: "beta", path: "beta.ns", text: "pub fn b() {}\n" },
    { key: "gamma", path: "gamma.ns", text: "pub fn c() {}\n" },
  ]);

  const result = spawnSync(binary, [], { input: req, encoding: "utf8" });
  assert.equal(result.status, 0, result.stderr);

  const response = JSON.parse(result.stdout);
  assert.equal(response.result.ok, true);

  const keys = response.result.modules.map((m) => m.key);
  assert.deepEqual(keys, ["alpha", "beta", "gamma"], "module order should match request order");

  assert.deepEqual(
    response.result.metadata.moduleOrder,
    ["alpha", "beta", "gamma"],
    "metadata.moduleOrder should match request order",
  );
});
