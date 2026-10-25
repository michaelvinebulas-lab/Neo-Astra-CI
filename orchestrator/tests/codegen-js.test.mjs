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

test("javascript backend implements the arbitrary integer policy", () => {
  const generated = generate(
    [
      "pub fn main() -> i32 {",
      "    let a: i32 = 2147483647",
      "    let b: i32 = 1",
      "    let sum: i32 = a + b",
      "    println(sum)",
      "    println(sum * sum)",
      "    println(a.wrapping_add(b))",
      "    let mut index: usize = 0",
      "    let values: [i32] = [10, 20, 30]",
      "    println(values[index])",
      "    let length: usize = values.len()",
      "    println(length)",
      '    match "42".to_int() {',
      "        Ok(value) => println(value)",
      '        Err(error) => println("parse error")',
      "    }",
      "    return 0",
      "}",
      "",
    ].join("\n"),
    { integerPolicy: "arbitrary" },
  );
  assert.deepEqual(generated.diagnostics, []);
  const result = runGenerated(generated.code);
  assert.equal(result.status, 0, result.stderr);
  assert.equal(
    result.stdout,
    "2147483648\n4611686018427387904\n-2147483648\n10\n3\n42\nRETURN=0",
  );
});

test("javascript backend implements checked integer methods", () => {
  const generated = generate(
    [
      "pub fn main() -> i32 {",
      "    let value: i32 = 2147483647",
      "    match value.checked_add(1) {",
      "        Ok(result) => println(result)",
      '        Err(error) => println("overflow")',
      "    }",
      "    let other: i32 = 7",
      "    match other.checked_mul(6) {",
      "        Ok(result) => println(result)",
      '        Err(error) => println("overflow")',
      "    }",
      "    let quotient: i32 = 9",
      "    match quotient.checked_div(2) {",
      "        Ok(result) => println(result)",
      '        Err(error) => println("overflow")',
      "    }",
      "    return 0",
      "}",
      "",
    ].join("\n"),
  );
  assert.deepEqual(generated.diagnostics, []);
  assert.match(generated.code, /function \$checkedInteger\(tag, operation, left, right\)/);

  const result = runGenerated(generated.code);
  assert.equal(result.status, 0, result.stderr);
  assert.equal(result.stdout, "overflow\n42\n4\nRETURN=0");
});

test("javascript backend runs spawned tasks eagerly and joins them", () => {
  const generated = generate(
    [
      "pub fn main() -> i32 {",
      "    let task = spawn || {",
      '        println("task")',
      "        21",
      "    }",
      '    println("after")',
      "    return task.join()",
      "}",
      "",
    ].join("\n"),
  );
  assert.deepEqual(generated.diagnostics, []);
  assert.match(generated.code, /function \$spawn\(body\)/);
  assert.match(generated.code, /function \$taskJoin\(task\)/);

  const result = runGenerated(generated.code);
  assert.equal(result.status, 0, result.stderr);
  assert.equal(result.stdout, "task\nafter\nRETURN=21");
});

test("javascript backend propagates panics from joined tasks", () => {
  const generated = generate(
    [
      "pub fn main() -> i32 {",
      '    let task = spawn || { panic("task boom") }',
      "    task.join()",
      "    return 0",
      "}",
      "",
    ].join("\n"),
  );
  assert.deepEqual(generated.diagnostics, []);

  const result = runGenerated(generated.code);
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /panic: task boom/);
});

test("javascript backend fails a scope for unjoined task panics", () => {
  const generated = generate(
    [
      "pub fn main() -> i32 {",
      '    spawn || { panic("scope boom") }',
      '    println("after")',
      "    return 0",
      "}",
      "",
    ].join("\n"),
  );
  assert.deepEqual(generated.diagnostics, []);

  const result = runGenerated(generated.code);
  assert.notEqual(result.status, 0);
  assert.equal(result.stdout, "after\n");
  assert.match(result.stderr, /panic: scope boom/);
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
  assert.match(
    generated.declarations,
    /declare function add\(a: number, b: number\): number;/,
  );
  assert.match(generated.declarations, /export \{ add as plus \};/);
  assert.match(generated.declarations, /export \{ add as default \};/);
  assert.match(generated.declarations, /declare function main\(\): number;/);

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

test("javascript backend executes recoverable failures and propagation", () => {
  const generated = generate(
    [
      "struct ParseError(str)",
      "",
      "fn reject() -> i32 ! ParseError {",
      '    return !ParseError("negative")',
      "}",
      "",
      "fn checked(value: i32) -> i32 ! ParseError {",
      "    return match value < 0 {",
      "        true => reject()!,",
      "        false => value,",
      "    }",
      "}",
      "",
      "pub fn main() -> i32 {",
      "    match checked(-1) {",
      '        Ok(value) => println(value)',
      '        Err(error) => println("caught")',
      "    }",
      "    match checked(2) {",
      "        Ok(value) => println(value)",
      '        Err(_) => println("unexpected")',
      "    }",
      '    match "not a number".to_int() {',
      "        Ok(value) => println(value)",
      '        Err(error) => println("parse error")',
      "    }",
      "    return 0",
      "}",
      "",
    ].join("\n"),
  );
  assert.deepEqual(generated.diagnostics, []);
  assert.match(generated.code, /function \$failure\(error\)/);
  assert.match(generated.code, /function \$fail\(error\)/);

  const result = runGenerated(generated.code);
  assert.equal(result.status, 0, result.stderr);
  assert.equal(result.stdout, "caught\n2\nparse error\nRETURN=0");
});

test("astra build emits a linked multi-module project with re-exports", () => {
  const project = mkdtempSync(join(tmpdir(), "astra-build-project-"));
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
      join(project, "src", "lib.ns"),
      [
        "module demo.lib",
        "",
        "pub fn add(a: i32, b: i32) -> i32 { return a + b }",
        "pub fn make() -> i32 { return 7 }",
        "export default make",
        "",
      ].join("\n"),
      "utf8",
    );
    writeFileSync(
      join(project, "src", "barrel.ns"),
      [
        "module demo.barrel",
        "",
        'export { add as plus } from "./lib"',
        'export * from "./lib"',
        'export * as math from "./lib"',
        "",
      ].join("\n"),
      "utf8",
    );
    writeFileSync(
      join(project, "src", "main.ns"),
      [
        "module demo.main",
        "",
        'import { add, plus, math } from "./barrel"',
        'import make from "./lib"',
        'import * as libMath from "./lib"',
        "",
        "pub fn main() -> i32 {",
        "    println(plus(2, 3))",
        "    println(libMath.add(4, 5))",
        "    println(math.add(6, 7))",
        "    println(make())",
        "    return add(1, 2)",
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

    const mainPath = join(project, "dist", "web", "src", "main.js");
    assert.ok(existsSync(mainPath));
    const mainCode = readFileSync(mainPath, "utf8");
    assert.match(mainCode, /import \{ add, plus, math \} from "\.\/barrel\.js";/);
    assert.match(mainCode, /import make from "\.\/lib\.js";/);
    assert.match(mainCode, /import \* as libMath from "\.\/lib\.js";/);

    const barrelCode = readFileSync(
      join(project, "dist", "web", "src", "barrel.js"),
      "utf8",
    );
    assert.match(barrelCode, /export \{ add as plus \} from "\.\/lib\.js";/);
    assert.match(barrelCode, /export \* from "\.\/lib\.js";/);
    assert.match(barrelCode, /export \* as math from "\.\/lib\.js";/);
    const mainDeclarations = readFileSync(
      join(project, "dist", "web", "src", "main.d.ts"),
      "utf8",
    );
    assert.match(mainDeclarations, /declare function main\(\): number;/);
    assert.match(mainDeclarations, /export \{ main \};/);

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
    assert.equal(run.stdout, "5\n9\n13\n7\nRETURN=3");
  } finally {
    rmSync(project, { recursive: true, force: true });
  }
});

test("astra build emits an inline JavaScript script module and calls it", () => {
  const project = mkdtempSync(join(tmpdir(), "astra-script-project-"));
  try {
    mkdirSync(join(project, "src"), { recursive: true });
    writeFileSync(
      join(project, "core.json"),
      JSON.stringify(
        {
          package: { name: "script-demo", version: "1.0.0", edition: "2026", license: "MIT" },
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
        "{@script:js as math;",
        "    @mode(core)",
        "    @exports {",
        "        fn add(a: i32, b: i32) -> i32",
        "        fn greet(name: str) -> str",
        "        fn maybe(flag: bool) -> Option<i32>",
        "        fn unwrap_or(value: Option<i32>, fallback: i32) -> i32",
        "    }",
        "",
        "    @impl",
        "    export function add(a, b) { return a + b }",
        '    export function greet(name) { return "hi " + name }',
        "    export function maybe(flag) { return flag ? 5 : null }",
        "    export function unwrap_or(value, fallback) { return value === null ? fallback : value }",
        "@end",
        "}",
        "",
        "pub fn main() -> i32 {",
        "    println(math.add(2, 3))",
        '    println(math.greet("neo"))',
        "    println(math.maybe(true))",
        "    println(math.unwrap_or(Some(4), 9))",
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

    const mainPath = join(project, "dist", "web", "src", "main.js");
    const mainCode = readFileSync(mainPath, "utf8");
    assert.match(mainCode, /import \* as \$script_math from "\.\/main\.math\.js";/);
    assert.match(mainCode, /\$script_math\.add\(arg0, arg1\)/);
    assert.match(mainCode, /arg0\.\$option \? arg0\.value : null/);

    const scriptPath = join(project, "dist", "web", "src", "main.math.js");
    const scriptCode = readFileSync(scriptPath, "utf8");
    assert.match(scriptCode, /export function add\(a, b\)/);
    assert.ok(existsSync(`${scriptPath}.map`));
    const scriptMap = JSON.parse(readFileSync(`${scriptPath}.map`, "utf8"));
    assert.match(scriptMap.sourcesContent[0], /export function add\(a, b\)/);
    const scriptDeclarations = readFileSync(
      join(project, "dist", "web", "src", "main.math.d.ts"),
      "utf8",
    );
    assert.match(
      scriptDeclarations,
      /export declare function add\(a: number, b: number\): number;/,
    );
    assert.match(
      scriptDeclarations,
      /export declare function maybe\(flag: boolean\): \(number \| null\);/,
    );

    const runnerPath = join(project, "dist", "web", "runner.mjs");
    writeFileSync(
      runnerPath,
      [
        'import { main } from "./src/main.js";',
        "const value = main();",
        'process.stdout.write("RETURN=" + String(value));',
        "",
      ].join("\n"),
      "utf8",
    );
    const run = spawnSync(process.execPath, [runnerPath], {
      cwd: join(project, "dist", "web"),
      encoding: "utf8",
    });
    assert.equal(run.status, 0, run.stderr);
    assert.equal(run.stdout, "5\nhi neo\nSome(5)\n4\nRETURN=0");
  } finally {
    rmSync(project, { recursive: true, force: true });
  }
});

test("astra build reports a missing script export", () => {
  const project = mkdtempSync(join(tmpdir(), "astra-script-missing-"));
  try {
    mkdirSync(join(project, "src"), { recursive: true });
    writeFileSync(
      join(project, "core.json"),
      JSON.stringify(
        {
          package: { name: "script-missing", version: "1.0.0", edition: "2026", license: "MIT" },
          build: { defaultTarget: "web", defaultMode: "core" },
          nova: { entry: "src/main.ns" },
          targets: {
            web: { backend: "js", module: "esm", mode: "core", outDir: "dist/web" },
          },
        },
        null,
        2,
      ),
      "utf8",
    );
    writeFileSync(
      join(project, "src", "main.ns"),
      [
        "{@script:js as math;",
        "    @mode(core)",
        "    @exports {",
        "        fn add(a: i32, b: i32) -> i32",
        "    }",
        "",
        "    @impl",
        "    export function subtract(a, b) { return a - b }",
        "@end",
        "}",
        "",
        "pub fn main() -> i32 {",
        "    return math.add(1, 2)",
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
    assert.equal(result.status, 1);
    const output = `${result.stdout}${result.stderr}`;
    assert.match(output, /NS7008/);
    assert.match(output, /does not export declared function 'add'/);
  } finally {
    rmSync(project, { recursive: true, force: true });
  }
});

test("astra build maps script exceptions into Nova recoverable errors", () => {
  const project = mkdtempSync(join(tmpdir(), "astra-script-error-"));
  try {
    mkdirSync(join(project, "src"), { recursive: true });
    writeFileSync(
      join(project, "core.json"),
      JSON.stringify(
        {
          package: { name: "script-error", version: "1.0.0", edition: "2026", license: "MIT" },
          build: { defaultTarget: "web", defaultMode: "core" },
          nova: { entry: "src/main.ns" },
          targets: {
            web: { backend: "js", module: "esm", mode: "core", outDir: "dist/web" },
          },
        },
        null,
        2,
      ),
      "utf8",
    );
    writeFileSync(
      join(project, "src", "main.ns"),
      [
        "struct ParseError(str)",
        "",
        "{@script:js as parser;",
        "    @mode(core)",
        "    @exports {",
        "        fn parse(input: str) -> i32 ! ParseError",
        "    }",
        "",
        "    @impl",
        "    export function parse(input) {",
        '        if (input === "bad") throw new Error("bad input")',
        "        return 7",
        "    }",
        "@end",
        "}",
        "",
        "pub fn main() -> i32 {",
        '    let result = match parser.parse("bad") {',
        "        Ok(value) => value,",
        "        Err(error) => { println(error); -1 }",
        "    }",
        "    return result",
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

    const runnerPath = join(project, "dist", "web", "runner.mjs");
    writeFileSync(
      runnerPath,
      [
        'import { main } from "./src/main.js";',
        "const value = main();",
        'process.stdout.write("RETURN=" + String(value));',
        "",
      ].join("\n"),
      "utf8",
    );
    const run = spawnSync(process.execPath, [runnerPath], {
      cwd: join(project, "dist", "web"),
      encoding: "utf8",
    });
    assert.equal(run.status, 0, run.stderr);
    assert.equal(run.stdout, "ParseError(bad input)\nRETURN=-1");
  } finally {
    rmSync(project, { recursive: true, force: true });
  }
});

test("astra build rejects a TypeScript script on the JavaScript target", () => {
  const project = mkdtempSync(join(tmpdir(), "astra-script-ts-"));
  try {
    mkdirSync(join(project, "src"), { recursive: true });
    writeFileSync(
      join(project, "core.json"),
      JSON.stringify(
        {
          package: { name: "script-ts", version: "1.0.0", edition: "2026", license: "MIT" },
          build: { defaultTarget: "web", defaultMode: "core" },
          nova: { entry: "src/main.ns" },
          targets: {
            web: { backend: "js", module: "esm", mode: "core", outDir: "dist/web" },
          },
        },
        null,
        2,
      ),
      "utf8",
    );
    writeFileSync(
      join(project, "src", "main.ns"),
      [
        "{@script:ts as math;",
        "    @mode(core)",
        "    @exports {",
        "        fn add(a: i32, b: i32) -> i32",
        "    }",
        "",
        "    @impl",
        "    export function add(a: number, b: number): number { return a + b }",
        "@end",
        "}",
        "",
        "pub fn main() -> i32 {",
        "    return math.add(1, 2)",
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
    assert.equal(result.status, 1);
    const output = `${result.stdout}${result.stderr}`;
    assert.match(output, /NS7008/);
    assert.match(output, /cannot be emitted by the JavaScript backend/);
  } finally {
    rmSync(project, { recursive: true, force: true });
  }
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
    const mainSource = [
      "module demo.main",
      "",
      "pub fn main() -> i32 {",
      '    println("hello from build")',
      "    return 0",
      "}",
      "",
    ].join("\n");
    writeFileSync(join(project, "src", "main.ns"), mainSource, "utf8");

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

    const mapPath = `${outputPath}.map`;
    assert.ok(existsSync(mapPath));
    assert.match(
      readFileSync(outputPath, "utf8"),
      /\/\/# sourceMappingURL=main\.js\.map/,
    );
    const sourceMap = JSON.parse(readFileSync(mapPath, "utf8"));
    assert.equal(sourceMap.version, 3);
    assert.equal(sourceMap.file, "main.js");
    assert.deepEqual(sourceMap.sourcesContent, [mainSource]);
    assert.ok(sourceMap.mappings.length > 0);

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
