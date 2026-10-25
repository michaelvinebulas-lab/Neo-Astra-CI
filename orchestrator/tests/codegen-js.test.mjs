import test from "node:test";
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import {
  existsSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { generateJavaScript } from "../packages/codegen-js/dist/index.js";

const root = fileURLToPath(new URL("../..", import.meta.url));
const binary = join(
  root,
  "target",
  "debug",
  process.platform === "win32" ? "astra-compiler.exe" : "astra-compiler",
);
const cli = join(root, "orchestrator", "packages", "cli", "dist", "index.js");

function compileSource(source, { mode = "core", integerPolicy = "wrapping" } = {}) {
  const request = JSON.stringify({
    jsonrpc: "2.0",
    id: "codegen",
    method: "compile",
    params: {
      apiVersion: "1.0",
      source: { path: "app/main.ns", text: source },
      target: "js",
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

function generate(source, options) {
  const compiled = compileSource(source, options);
  assert.deepEqual(
    compiled.diagnostics,
    [],
    `unexpected frontend diagnostics: ${JSON.stringify(compiled.diagnostics)}`,
  );
  assert.notEqual(compiled.ir, undefined);
  return generateJavaScript({ moduleName: "app.main", ir: compiled.ir });
}

function runGeneratedWith(code, runnerLines) {
  const directory = mkdtempSync(join(tmpdir(), "astra-codegen-"));
  try {
    writeFileSync(join(directory, "generated.mjs"), code, "utf8");
    writeFileSync(
      join(directory, "runner.mjs"),
      ['import * as module from "./generated.mjs";', ...runnerLines, ""].join("\n"),
      "utf8",
    );
    return spawnSync(process.execPath, [join(directory, "runner.mjs")], {
      cwd: directory,
      encoding: "utf8",
    });
  } finally {
    rmSync(directory, { recursive: true, force: true });
  }
}

function runGenerated(code) {
  return runGeneratedWith(code, [
    "const value = module.main();",
    'process.stdout.write("RETURN=" + String(value));',
  ]);
}

test("javascript backend executes structs, enums, options, maps, and defer", () => {
  const source = [
    "module app.main",
    "",
    "struct Point { x: i32, y: i32 }",
    "struct Wrap(i32)",
    "enum Status { Active, Inactive(reason: str) }",
    "",
    "fn describe(status: Status) -> str {",
    "    return match status {",
    '        Status.Active => "active",',
    "        Status.Inactive(reason) => reason,",
    "    }",
    "}",
    "",
    "fn total(values: [i32]) -> i32 {",
    "    let mut sum = 0",
    "    for value in values {",
    "        sum = sum + value",
    "    }",
    "    return sum",
    "}",
    "",
    "pub fn main() -> i32 {",
    "    let point = Point { x: 3, y: 4 }",
    "    println(point.x)",
    "    let wrapped = Wrap(9)",
    "    println(wrapped.0)",
    "    println(describe(Status.Active))",
    '    println(describe(Status.Inactive("paused")))',
    "    let values = [1, 2, 3]",
    "    values.push(4)",
    "    println(total(values))",
    "    let maybe: Option<i32> = Some(42)",
    "    println(maybe)",
    "    println(maybe.unwrap())",
    "    let empty: Option<i32> = None",
    "    println(empty.unwrap_or(8))",
    '    let map = {"a": 1}',
    '    println(map.get("a"))',
    '    println(map.get("missing"))',
    "    println((1, \"x\"))",
    "    {",
    '        defer println("deferred")',
    '        println("inside")',
    "    }",
    "    return total(values)",
    "}",
    "",
  ].join("\n");

  const generated = generate(source);
  assert.deepEqual(generated.diagnostics, []);
  assert.match(generated.code, /function Point\(fields\)/);
  assert.match(generated.code, /const Status = Object\.freeze/);

  const result = runGenerated(generated.code);
  assert.equal(result.status, 0, result.stderr);
  assert.equal(
    result.stdout,
    [
      "3",
      "9",
      "active",
      "paused",
      "10",
      "Some(42)",
      "42",
      "8",
      "Some(1)",
      "None",
      "(1, x)",
      "inside",
      "deferred",
      "RETURN=10",
    ].join("\n"),
  );
});

test("javascript backend implements wrapping and checked integer policies", () => {
  const wrapping = generate(
    [
      "pub fn main() -> i32 {",
      "    let mut value: i32 = 2147483647",
      "    value = value + 1",
      "    println(value)",
      "    let byte: u8 = 255",
      "    println(byte + 1u8)",
      "    return value",
      "}",
      "",
    ].join("\n"),
    { integerPolicy: "wrapping" },
  );
  assert.deepEqual(wrapping.diagnostics, []);
  const wrapResult = runGenerated(wrapping.code);
  assert.equal(wrapResult.status, 0, wrapResult.stderr);
  assert.equal(wrapResult.stdout, "-2147483648\n0\nRETURN=-2147483648");

  const checked = generate(
    [
      "pub fn main() -> i32 {",
      "    let mut value: i32 = 2147483647",
      "    value = value + 1",
      "    return value",
      "}",
      "",
    ].join("\n"),
    { integerPolicy: "safe-checked" },
  );
  assert.deepEqual(checked.diagnostics, []);
  const checkedResult = runGenerated(checked.code);
  assert.notEqual(checkedResult.status, 0);
  assert.match(checkedResult.stderr, /integer overflow/);
});

test("javascript backend preserves float values and rendering", () => {
  const generated = generate(
    [
      "pub fn main() -> i32 {",
      "    let a: f64 = 1.0",
      "    let b: f64 = 2.5",
      "    println(a)",
      "    println(a + b)",
      "    let c: f32 = 1.5",
      "    println(c)",
      "    println(a == 1.0)",
      "    return 0",
      "}",
      "",
    ].join("\n"),
  );
  assert.deepEqual(generated.diagnostics, []);
  const result = runGenerated(generated.code);
  assert.equal(result.status, 0, result.stderr);
  assert.equal(result.stdout, "1.0\n3.5\n1.5\ntrue\nRETURN=0");
});

test("javascript backend renders byte strings", () => {
  const generated = generate(
    [
      "pub fn main() -> i32 {",
      '    println(b"hi\\n")',
      "    return 0",
      "}",
      "",
    ].join("\n"),
  );
  assert.deepEqual(generated.diagnostics, []);
  const result = runGenerated(generated.code);
  assert.equal(result.status, 0, result.stderr);
  assert.equal(result.stdout, 'b"hi\\n"\nRETURN=0');
});

test("javascript backend emits named and default exports", () => {
  const generated = generate(
    [
      "fn add(a: i32, b: i32) -> i32 { return a + b }",
      "const ANSWER: i32 = 42",
      "export { add as plus }",
      "export default add",
      "pub fn main() -> i32 { return ANSWER }",
      "",
    ].join("\n"),
  );
  assert.deepEqual(generated.diagnostics, []);
  assert.match(generated.code, /export \{ add as plus \};/);
  assert.match(generated.code, /export default add;/);

  const result = runGeneratedWith(generated.code, [
    'process.stdout.write(String(module.plus(2, 3)) + "," + String(module.default(4, 5)) + "," + String(module.main()));',
  ]);
  assert.equal(result.status, 0, result.stderr);
  assert.equal(result.stdout, "5,9,42");
});

test("javascript backend emits expression default exports", () => {
  const generated = generate(
    [
      "pub fn main() -> i32 { return 0 }",
      "export default 7",
      "",
    ].join("\n"),
  );
  assert.deepEqual(generated.diagnostics, []);
  assert.match(generated.code, /export default 7;/);

  const result = runGeneratedWith(generated.code, [
    'process.stdout.write(String(module.default));',
  ]);
  assert.equal(result.status, 0, result.stderr);
  assert.equal(result.stdout, "7");
});

test("javascript backend rejects non-local control flow inside expression blocks", () => {
  const generated = generate(
    [
      "fn choose(value: i32) -> i32 {",
      "    match value {",
      "        1 => { return 10 }",
      "        _ => { return 20 }",
      "    }",
      "    return 0",
      "}",
      "pub fn main() -> i32 { return choose(1) }",
      "",
    ].join("\n"),
  );
  assert.ok(
    generated.diagnostics.some(
      (diagnostic) =>
        diagnostic.code === "NS8001" &&
        diagnostic.message.includes("Control flow that exits an expression block"),
    ),
  );
});

test("javascript backend rejects project imports explicitly", () => {
  const generated = generate('import "./other"\npub fn main() {}\n');
  assert.ok(
    generated.diagnostics.some(
      (diagnostic) =>
        diagnostic.code === "NS8001" &&
        diagnostic.message.includes("Module imports"),
    ),
  );
});

test("javascript backend rejects manual memory explicitly", () => {
  const generated = generate(
    "pub fn f() { {@memory:manual;\nlet x = 1\n} }\npub fn main() {}\n",
    { mode: "system" },
  );
  assert.ok(
    generated.diagnostics.some(
      (diagnostic) =>
        diagnostic.code === "NS8003" &&
        diagnostic.message.includes("Manual memory"),
    ),
  );
});

test("javascript backend rejects error propagation explicitly", () => {
  const generated = generate(
    [
      "fn parse() -> i32 ! ParseIntError {",
      '    return "1".to_int()!',
      "}",
      "pub fn main() -> i32 ! ParseIntError { return parse()! }",
      "",
    ].join("\n"),
  );
  assert.ok(
    generated.diagnostics.some(
      (diagnostic) =>
        diagnostic.code === "NS8001" &&
        diagnostic.message.includes("Error propagation"),
    ),
  );
});

test("astra build writes runnable ESM and a module package marker", () => {
  const project = mkdtempSync(join(tmpdir(), "astra-build-"));
  try {
    mkdirSync(join(project, "src"), { recursive: true });
    writeFileSync(
      join(project, "core.json"),
      JSON.stringify(
        {
          package: { name: "demo", version: "1.0.0", edition: "2026", license: "MIT" },
          build: { defaultTarget: "web", defaultMode: "core" },
          nova: { entry: "src/main.ns" },
          targets: {
            web: { backend: "js", module: "esm", mode: "core", outDir: "dist/web" },
          },
          math: { integers: "wrapping" },
        },
        null,
        2,
      ),
      "utf8",
    );
    writeFileSync(
      join(project, "src", "main.ns"),
      [
        "module demo.main",
        "",
        "pub fn main() -> i32 {",
        '    println("hello from build")',
        "    return 0",
        "}",
        "",
      ].join("\n"),
      "utf8",
    );

    const result = spawnSync(
      process.execPath,
      [cli, "build", project, "--compiler", binary],
      { cwd: root, encoding: "utf8" },
    );
    assert.equal(result.status, 0, result.stderr);
    assert.match(result.stdout, /built web/);

    const outputPath = join(project, "dist", "web", "src", "main.js");
    assert.ok(existsSync(outputPath));
    assert.equal(
      readFileSync(join(project, "dist", "web", "package.json"), "utf8"),
      '{ "type": "module" }\n',
    );

    writeFileSync(
      join(project, "dist", "web", "runner.mjs"),
      [
        'import { main } from "./src/main.js";',
        "const value = main();",
        'process.stdout.write("RETURN=" + String(value));',
        "",
      ].join("\n"),
      "utf8",
    );
    const run = spawnSync(
      process.execPath,
      [join(project, "dist", "web", "runner.mjs")],
      { cwd: join(project, "dist", "web"), encoding: "utf8" },
    );
    assert.equal(run.status, 0, run.stderr);
    assert.equal(run.stdout, "hello from build\nRETURN=0");
  } finally {
    rmSync(project, { recursive: true, force: true });
  }
});
