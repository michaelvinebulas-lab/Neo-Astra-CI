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
import { generateTypeScript, PINNED_TYPESCRIPT_VERSION, pinnedTypeScriptVersionError } from "../packages/codegen-ts/dist/index.js";
import { generateJavaScript, generateModule } from "../packages/codegen-js/dist/index.js";
import {
  loadConformanceCases,
  normalizeConformanceText,
} from "./conformance.mjs";

const root = fileURLToPath(new URL("../..", import.meta.url));
const binary = join(
  root,
  "target",
  "debug",
  process.platform === "win32" ? "astra-compiler.exe" : "astra-compiler",
);
const cli = join(root, "orchestrator", "packages", "cli", "dist", "index.js");
const tscPath = join(root, "node_modules", "typescript", "lib", "tsc.js");

function compileSource(
  source,
  { mode = "core", integerPolicy = "wrapping", target = "ts", edition } = {},
) {
  const request = JSON.stringify({
    jsonrpc: "2.0",
    id: "codegen-ts",
    method: "compile",
    params: {
      apiVersion: "1.0",
      source: { path: "app/main.ns", text: source },
      target,
      mode,
      integerPolicy,
      ...(edition === undefined ? {} : { edition }),
    },
  });

  const result = spawnSync(binary, [], { input: request, encoding: "utf8" });
  assert.equal(result.status, 0, result.stderr);
  const response = JSON.parse(result.stdout);
  assert.deepEqual(response.error, undefined);
  return response.result;
}

function generateFor(target, source, options) {
  const compiled = compileSource(source, { ...options, target });
  assert.deepEqual(
    compiled.diagnostics,
    [],
    `unexpected frontend diagnostics: ${JSON.stringify(compiled.diagnostics)}`,
  );
  assert.notEqual(compiled.ir, undefined);
  return target === "ts"
    ? generateTypeScript({ moduleName: "app.main", ir: compiled.ir, sourceText: source })
    : generateJavaScript({ moduleName: "app.main", ir: compiled.ir, sourceText: source });
}

function runTsc(directory, args) {
  const result = spawnSync(
    process.execPath,
    [tscPath, "--strict", "--target", "es2022", "--module", "nodenext", "--moduleResolution", "nodenext", ...args],
    { cwd: directory, encoding: "utf8" },
  );
  const output = `${result.stdout ?? ""}${result.stderr ?? ""}`;
  return { status: result.status, errors: output.split(/\r?\n/).filter((line) => line.includes("error TS")) };
}

function writeArtifacts(directory, generated) {
  for (const artifact of generated.artifacts ?? []) {
    const file = join(directory, artifact.path.split("/").pop());
    writeFileSync(file, artifact.code, "utf8");
    if (artifact.declarations !== undefined) {
      writeFileSync(file.replace(/\.(js|ts)$/i, ".d.ts"), artifact.declarations, "utf8");
    }
  }
}

function runNode(directory, file) {
  return spawnSync(process.execPath, [join(directory, file)], {
    cwd: directory,
    encoding: "utf8",
  });
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

  assert.doesNotMatch(generated.code, /\/\/ Target: TypeScript ESM\./);
  assert.match(generated.code, /function add\(a: number, b: number\): number \{/);
  assert.match(generated.code, /function main\(\): number \{/);
  assert.match(generated.code, /export \{ add \};/);
  assert.match(generated.code, /export \{ main \};/);
  // Strict TypeScript: no implicit-any annotations anywhere in the output.
  assert.doesNotMatch(generated.code, /:\s*any\b/);
});

const parityPrograms = [
  {
    name: "kitchen sink",
    source: [
      "module app.main",
      "",
      "struct Point { x: i32, y: i32 }",
      "struct Wrap(i32)",
      "enum Status { Active, Inactive(reason: str) }",
      "struct ParseError(str)",
      "",
      "const MAX: i32 = 100",
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
      '    println((1, "x"))',
      "    match checked(-1) {",
      "        Ok(value) => println(value)",
      '        Err(error) => println("caught")',
      "    }",
      '    match "not a number".to_int() {',
      "        Ok(value) => println(value)",
      '        Err(error) => println("parse error")',
      "    }",
      "    {",
      '        defer println("deferred")',
      '        println("inside")',
      "    }",
      "    let mut i: i32 = 0",
      "    while i < 3 {",
      "        i = i + 1",
      "    }",
      "    println(i)",
      "    println(MAX)",
      '    if point.x > 0 { println("pos") } else { println("neg") }',
      "    return total(values)",
      "}",
      "",
    ].join("\n"),
  },
  {
    name: "runtime features",
    source: [
      "fn produce(channel: Channel<str>) -> () ! ChannelError {",
      '    channel.send("hello")!',
      '    channel.send("world")!',
      "}",
      "",
      "pub fn main() -> i32 {",
      "    let a: f64 = 1.0",
      "    let b: f64 = 2.5",
      "    println(a)",
      "    println(a + b)",
      "    let c: f32 = 1.5",
      "    println(c)",
      "    println(a == 1.0)",
      "",
      '    let text = "hAcllo"',
      "    println(text.len())",
      "    println(text.char_count())",
      "    match text.byte_at(1) {",
      "        Some(byte) => println(byte),",
      '        None => println("none"),',
      "    }",
      "    match text.char_at(1) {",
      "        Some(character) => println(character),",
      '        None => println("none"),',
      "    }",
      "    match text.slice(0, 3) {",
      "        Ok(part) => println(part),",
      '        Err(error) => println("slice failed"),',
      "    }",
      "    match text.slice(1, 99) {",
      "        Ok(part) => println(part),",
      '        Err(error) => println("oob"),',
      "    }",
      "    println(text.bytes().len())",
      "    println(text.chars().len())",
      "    println(text.chars()[1])",
      '    println(b"hi\\\\n")',
      "",
      '    let mut map = {"a": 1}',
      '    map.insert("b", 2)',
      "    println(map.len())",
      '    println(map.get("b"))',
      '    println(map.get("a"))',
      '    println(map.get("missing"))',
      "",
      "    let value: i32 = 2147483647",
      "    match value.checked_add(1) {",
      "        Ok(result) => println(result),",
      '        Err(error) => println("overflow"),',
      "    }",
      "    let mut wrapped: i32 = 2147483647",
      "    wrapped = wrapped + 1",
      "    println(wrapped)",
      "    let byteMax: u8 = 255",
      "    println(byteMax + 1u8)",
      "",
      "    let task = spawn || {",
      '        println("task")',
      "        21",
      "    }",
      '    println("after")',
      "    println(task.join())",
      "",
      "    let channel = Channel.create<str>()",
      "    spawn || {",
      "        match produce(channel) {",
      '            Ok(value) => println("sent"),',
      '            Err(error) => println("send failed"),',
      "        }",
      "    }",
      "    match channel.receive() {",
      "        Ok(message) => println(message),",
      '        Err(error) => println("closed"),',
      "    }",
      "    match channel.receive() {",
      "        Ok(message) => println(message),",
      '        Err(error) => println("closed"),',
      "    }",
      "    channel.close()",
      '    match channel.send("late") {',
      '        Ok(value) => println("sent late"),',
      '        Err(error) => println("closed send"),',
      "    }",
      "    match channel.receive() {",
      "        Ok(message) => println(message),",
      '        Err(error) => println("closed recv"),',
      "    }",
      "    return 0",
      "}",
      "",
    ].join("\n"),
  },
  {
    name: "arbitrary integer policy",
    integerPolicy: "arbitrary",
    source: [
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
  },
  {
    name: "safe-checked integer policy",
    integerPolicy: "safe-checked",
    source: [
      "pub fn main() -> i32 {",
      "    let a: i32 = 100",
      "    let b: i32 = 23",
      "    let sum: i32 = a + b",
      "    println(sum)",
      "    let product: i32 = sum * 2",
      "    println(product)",
      "    let difference: i32 = product - 46",
      "    println(difference)",
      "    match product.checked_div(7) {",
      "        Ok(result) => println(result),",
      '        Err(error) => println("error"),',
      "    }",
      "    return 0",
      "}",
      "",
    ].join("\n"),
  },
  {
    name: "inline script block",
    source: [
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
  },
  {
    name: "exclusive and inclusive ranges",
    edition: "0.2",
    source: [
      "pub fn main() -> i32 {",
      "    let mut total = 0",
      "    for value in 0..3 {",
      "        total = total + value",
      "    }",
      "    for value in 0..=3 {",
      "        total = total + value",
      "    }",
      "    let values = [10, 20, 30, 40]",
      "    println(values[0..2].len())",
      "    println(values[1..=2].len())",
      "    println(0b1100 & 0b1010)",
      "    println(0b1000 | 0b0010)",
      "    println(0b1010 ^ 0b1111)",
      "    println(1 << 4)",
      "    println(256 >> 4)",
      "    println(total)",
      "    return total",
      "}",
      "",
    ].join("\n"),
  },
  {
    name: "char and scalar literals",
    edition: "0.2",
    source: [
      "pub fn main() -> i32 {",
      "    let letter = 'a'",
      "    println(letter)",
      "    println('\\u{1F600}')",
      '    println(match letter { \'a\' => "first", _ => "other" })',
      "    return 0",
      "}",
      "",
    ].join("\n"),
  },
  {
    name: "struct field mutation",
    source: [
      "struct User {",
      "    name: str,",
      "    age: i32,",
      "}",
      "struct Pair(i32, i32)",
      "",
      "pub fn main() -> i32 {",
      '    let mut user = User { name: "Nova", age: 1 }',
      '    user.name = "Neo"',
      "    user.age = user.age + 41",
      "    println(user.name)",
      "    println(user.age)",
      "    let mut pair = Pair(2, 3)",
      "    pair.0 = pair.0 + pair.1",
      "    println(pair.0)",
      "    return 0",
      "}",
      "",
    ].join("\n"),
  },
];

test("emitted TypeScript passes tsc --strict, runs, and matches the JavaScript backend", () => {
  for (const program of parityPrograms) {
    const options = {
      ...(program.integerPolicy === undefined ? {} : { integerPolicy: program.integerPolicy }),
      ...(program.edition === undefined ? {} : { edition: program.edition }),
    };
    const tsGenerated = generateFor("ts", program.source, options);
    const jsGenerated = generateFor("js", program.source, options);
    assert.deepEqual(tsGenerated.diagnostics, [], `${program.name}: ${JSON.stringify(tsGenerated.diagnostics)}`);
    assert.deepEqual(jsGenerated.diagnostics, [], `${program.name}: ${JSON.stringify(jsGenerated.diagnostics)}`);

    const directory = mkdtempSync(join(tmpdir(), "astra-ts-check-"));
    try {
      writeFileSync(join(directory, "package.json"), JSON.stringify({ type: "module" }), "utf8");
      writeFileSync(join(directory, "main.ts"), tsGenerated.code, "utf8");
      writeArtifacts(directory, tsGenerated);

      const check = runTsc(directory, ["--noEmit", "main.ts"]);
      if (check.status !== 0 || check.errors.length > 0) {
        const codeLines = tsGenerated.code.split("\n");
        const context = check.errors.flatMap((line) => {
          const match = line.match(/main\.ts\((\d+),/);
          if (match === null) return [line];
          const number = Number(match[1]);
          const from = Math.max(0, number - 3);
          const to = Math.min(codeLines.length, number + 2);
          return [
            line,
            ...codeLines.slice(from, to).map(
              (text, offset) => `  ${from + offset + 1}${from + offset + 1 === number ? ">>" : "  "} ${text}`,
            ),
          ];
        });
        assert.fail(
          `${program.name}: tsc --strict failed:\n${context.join("\n")}`,
        );
      }

      const emit = runTsc(directory, ["--outDir", "dist", "main.ts"]);
      assert.equal(emit.status, 0, `${program.name}: tsc emit failed:\n${emit.errors.join("\n")}`);
      writeArtifacts(join(directory, "dist"), tsGenerated);
      writeFileSync(
        join(directory, "runner.mjs"),
        [
          'import { main } from "./dist/main.js";',
          'process.stdout.write("RETURN=" + String(main()));',
          "",
        ].join("\n"),
        "utf8",
      );
      const tsRun = runNode(directory, "runner.mjs");
      assert.equal(tsRun.status, 0, `${program.name}: ${tsRun.stderr}`);

      writeFileSync(join(directory, "generated.mjs"), jsGenerated.code, "utf8");
      writeArtifacts(directory, jsGenerated);
      writeFileSync(
        join(directory, "js-runner.mjs"),
        [
          'import * as module from "./generated.mjs";',
          'process.stdout.write("RETURN=" + String(module.main()));',
          "",
        ].join("\n"),
        "utf8",
      );
      const jsRun = runNode(directory, "js-runner.mjs");
      assert.equal(jsRun.status, 0, `${program.name}: ${jsRun.stderr}`);

      assert.equal(
        tsRun.stdout,
        jsRun.stdout,
        `${program.name}: TypeScript output diverged from the JavaScript backend`,
      );
    } finally {
      rmSync(directory, { recursive: true, force: true });
    }
  }
});

test("TypeScript backend passes the shared conformance suite", () => {
  const cases = loadConformanceCases();
  assert.ok(cases.length >= 4, "expected the shared conformance suite to have cases");
  for (const conformanceCase of cases) {
    const generated = generateFor("ts", conformanceCase.source);
    assert.deepEqual(
      generated.diagnostics,
      [],
      `${conformanceCase.name}: ${JSON.stringify(generated.diagnostics)}`,
    );

    const directory = mkdtempSync(join(tmpdir(), "astra-ts-conformance-"));
    try {
      writeFileSync(join(directory, "package.json"), JSON.stringify({ type: "module" }), "utf8");
      writeFileSync(join(directory, "main.ts"), generated.code, "utf8");

      const check = runTsc(directory, ["--noEmit", "main.ts"]);
      assert.equal(
        check.status,
        0,
        `${conformanceCase.name}: tsc --strict failed:\n${check.errors.join("\n")}`,
      );

      const emit = runTsc(directory, ["--outDir", "dist", "main.ts"]);
      assert.equal(emit.status, 0, `${conformanceCase.name}: tsc emit failed:\n${emit.errors.join("\n")}`);
      writeFileSync(
        join(directory, "runner.mjs"),
        [
          'import { main } from "./dist/main.js";',
          'process.stdout.write("RETURN=" + String(main()));',
          "",
        ].join("\n"),
        "utf8",
      );
      const run = runNode(directory, "runner.mjs");
      assert.equal(run.status, 0, `${conformanceCase.name}: ${run.stderr}`);
      assert.equal(
        normalizeConformanceText(run.stdout),
        conformanceCase.expected,
        conformanceCase.name,
      );
    } finally {
      rmSync(directory, { recursive: true, force: true });
    }
  }
});

test("astra build --target ts writes linked TypeScript that tsc accepts and Node runs", () => {
  const project = mkdtempSync(join(tmpdir(), "astra-ts-build-"));
  try {
    mkdirSync(join(project, "src"), { recursive: true });
    writeFileSync(
      join(project, "core.json"),
      JSON.stringify(
      {
        "package": {
          "name": "demo",
          "version": "1.0.0",
          "edition": "2026",
          "license": "MIT"
        }
      },
        null,
        2,
      ),
      "utf8",
    );

    writeFileSync(
      join(project, "build.json"),
      JSON.stringify(
      {
        "build": {
          "defaultTarget": "web",
          "defaultMode": "core"
        },
        "nova": {
          "entry": "src/main.ns"
        },
        "targets": {
          "web": {
            "backend": "js",
            "module": "esm",
            "mode": "core",
            "outDir": "dist/web"
          }
        },
        "math": {
          "integers": "wrapping"
        }
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
      [cli, "build", project, "--target", "ts", "--compiler", binary, "--json"],
      { cwd: root, encoding: "utf8" },
    );
    assert.equal(result.status, 0, result.stderr);
    const report = JSON.parse(result.stdout);
    assert.equal(report.backend, "ts");
    assert.equal(report.target, "ts");
    assert.deepEqual(report.diagnostics, []);
    assert.match(report.output, /main\.ts$/);

    const outDir = join(project, "dist", "ts");
    const mainPath = join(outDir, "src", "main.ts");
    assert.ok(existsSync(mainPath));
    assert.ok(existsSync(join(outDir, "src", "barrel.ts")));
    assert.ok(existsSync(join(outDir, "src", "lib.ts")));
    assert.ok(existsSync(`${mainPath}.map`));
    assert.ok(existsSync(join(outDir, "src", "main.d.ts")));
    assert.ok(existsSync(join(outDir, "package.json")));

    const mainCode = readFileSync(mainPath, "utf8");
    assert.doesNotMatch(mainCode, /\/\/ Target: TypeScript ESM\./);
    // Cross-module specifiers stay `.js` so nodenext resolution finds the
    // sibling `.ts` sources now and the tsc-emitted `.js` files at runtime.
    assert.match(mainCode, /from "\.\/barrel\.js";/);
    assert.match(mainCode, /from "\.\/lib\.js";/);
    assert.doesNotMatch(mainCode, /from "\.\/barrel\.ts";/);

    const declarations = readFileSync(join(outDir, "src", "main.d.ts"), "utf8");
    assert.match(declarations, /declare function main\(\): number;/);
    assert.match(declarations, /export \{ main \};/);

    const check = runTsc(outDir, ["--noEmit", join("src", "main.ts")]);
    assert.equal(
      check.status,
      0,
      `tsc --strict failed:\n${check.errors.join("\n")}`,
    );

    const emit = runTsc(outDir, ["--outDir", "compiled", join("src", "main.ts")]);
    assert.equal(emit.status, 0, `tsc emit failed:\n${emit.errors.join("\n")}`);
    assert.ok(existsSync(join(outDir, "compiled", "main.js")));

    writeFileSync(
      join(outDir, "runner.mjs"),
      [
        'import { main } from "./compiled/main.js";',
        'process.stdout.write("RETURN=" + String(main()));',
        "",
      ].join("\n"),
      "utf8",
    );
    const run = runNode(outDir, "runner.mjs");
    assert.equal(run.status, 0, run.stderr);
    assert.equal(run.stdout, "5\n9\n13\n7\nRETURN=3");
  } finally {
    rmSync(project, { recursive: true, force: true });
  }
});

// The same Nova program, once with a TypeScript `@script:ts` body and once with
// an equivalent JavaScript `@script:js` body. They must produce identical
// runtime output; only the script body's language and types differ.
const scriptTsSource = [
  "{@script:ts as math;",
  "    @mode(core)",
  "    @exports {",
  "        fn add(a: i32, b: i32) -> i32",
  "        fn label(value: i32) -> str",
  "    }",
  "",
  "    @impl",
  "    const prefix: string = 'v=';",
  "    export function add(a: number, b: number): number { return a + b; }",
  "    export function label(value: number): string { return prefix + String(value); }",
  "@end",
  "}",
  "",
  "pub fn main() -> i32 {",
  "    println(math.add(2, 3))",
  "    println(math.label(7))",
  "    return 0",
  "}",
  "",
].join("\n");

const scriptJsSource = scriptTsSource
  .replace("@script:ts", "@script:js")
  .replace("const prefix: string = 'v=';", "const prefix = 'v=';")
  .replace(
    "export function add(a: number, b: number): number { return a + b; }",
    "export function add(a, b) { return a + b; }",
  )
  .replace(
    "export function label(value: number): string { return prefix + String(value); }",
    "export function label(value) { return prefix + String(value); }",
  );

const BASE64_VLQ =
  "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

function decodeFirstMappingSegment(mappings) {
  // Skip the empty generated lines that carry no mappings (the two header
  // lines), then decode the very first segment of the first mapped line.
  const lines = mappings.split(";");
  const index = lines.findIndex((line) => line.length > 0);
  const first = lines[index].split(",")[0];
  const values = [];
  let shift = 0;
  let value = 0;
  for (const char of first) {
    const digit = BASE64_VLQ.indexOf(char);
    value += (digit & 31) << shift;
    if ((digit & 32) !== 0) {
      shift += 5;
      continue;
    }
    const negative = (value & 1) === 1;
    value >>= 1;
    values.push(negative ? -value : value);
    shift = 0;
    value = 0;
  }
  return { generatedLine: index, sourceLine: values[2] };
}

test("@script:ts transpiles with the pinned compiler, runs, and matches @script:js", () => {
  const tsGenerated = generateFor("ts", scriptTsSource);
  const jsGenerated = generateFor("js", scriptJsSource);
  assert.deepEqual(tsGenerated.diagnostics, [], JSON.stringify(tsGenerated.diagnostics));
  assert.deepEqual(jsGenerated.diagnostics, [], JSON.stringify(jsGenerated.diagnostics));

  const artifact = (tsGenerated.artifacts ?? [])[0];
  assert.ok(artifact, "expected a transpiled @script:ts artifact");
  // The transpiler strips the TypeScript annotations.
  assert.match(artifact.code, /export function add\(a, b\)/);
  assert.doesNotMatch(artifact.code, /: number/);

  const directory = mkdtempSync(join(tmpdir(), "astra-ts-script-"));
  try {
    writeFileSync(join(directory, "package.json"), JSON.stringify({ type: "module" }), "utf8");
    writeFileSync(join(directory, "main.ts"), tsGenerated.code, "utf8");
    writeArtifacts(directory, tsGenerated);

    const check = runTsc(directory, ["--noEmit", "main.ts"]);
    assert.equal(check.status, 0, `tsc --strict failed:\n${check.errors.join("\n")}`);

    const emit = runTsc(directory, ["--outDir", "dist", "main.ts"]);
    assert.equal(emit.status, 0, `tsc emit failed:\n${emit.errors.join("\n")}`);
    writeArtifacts(join(directory, "dist"), tsGenerated);
    writeFileSync(
      join(directory, "runner.mjs"),
      [
        'import { main } from "./dist/main.js";',
        'process.stdout.write("RETURN=" + String(main()));',
        "",
      ].join("\n"),
      "utf8",
    );
    const tsRun = runNode(directory, "runner.mjs");
    assert.equal(tsRun.status, 0, tsRun.stderr);
    assert.equal(tsRun.stdout, "5\nv=7\nRETURN=0");

    writeFileSync(join(directory, "generated.mjs"), jsGenerated.code, "utf8");
    writeArtifacts(directory, jsGenerated);
    writeFileSync(
      join(directory, "js-runner.mjs"),
      [
        'import * as module from "./generated.mjs";',
        'process.stdout.write("RETURN=" + String(module.main()));',
        "",
      ].join("\n"),
      "utf8",
    );
    const jsRun = runNode(directory, "js-runner.mjs");
    assert.equal(jsRun.status, 0, jsRun.stderr);

    assert.equal(
      tsRun.stdout,
      jsRun.stdout,
      "@script:ts output diverged from the equivalent @script:js program",
    );
  } finally {
    rmSync(directory, { recursive: true, force: true });
  }
});

test("@script:ts artifact carries a source map that points back at the .ns body", () => {
  const generated = generateFor("ts", scriptTsSource);
  assert.deepEqual(generated.diagnostics, [], JSON.stringify(generated.diagnostics));
  const artifact = (generated.artifacts ?? [])[0];
  assert.ok(artifact);
  assert.ok(artifact.sourceMap, "expected a composed source map on the transpiled artifact");

  const map = JSON.parse(artifact.sourceMap);
  assert.equal(map.version, 3);
  assert.deepEqual(map.sources, ["app/main.ns"]);
  assert.deepEqual(map.sourcesContent, [scriptTsSource]);

  // The body's first line is `const prefix: string = ...`; find its index in
  // the .ns source and check the map's first segment points there.
  const expectedLine = scriptTsSource
    .split("\n")
    .findIndex((line) => line.includes("const prefix"));
  const { generatedLine, sourceLine } = decodeFirstMappingSegment(map.mappings);
  // Two generated lines of header precede the transpiled body.
  assert.equal(generatedLine, 2);
  assert.equal(sourceLine, expectedLine);
});

test("@script:ts without a pinned transpiler is a hard NS7008 error, not a fallback", () => {
  const compiled = compileSource(scriptTsSource, { target: "ts" });
  assert.deepEqual(compiled.diagnostics, [], JSON.stringify(compiled.diagnostics));
  // Drive the shared emitter directly with backend "ts" but no transpiler
  // hook, which is what a caller that failed to wire the pinned toolchain
  // would look like.
  const generated = generateModule({
    moduleName: "app.main",
    ir: compiled.ir,
    sourceText: scriptTsSource,
    backend: "ts",
  });
  const ns7008 = generated.diagnostics.filter((d) => d.code === "NS7008");
  assert.equal(ns7008.length, 1, JSON.stringify(generated.diagnostics));
  assert.match(ns7008[0].message, /no pinned TypeScript transpiler is available/);
  assert.match(ns7008[0].message, /no best-effort fallback/);
  assert.equal((generated.artifacts ?? []).length, 0);
});

test("a syntactically invalid @script:ts body is a hard NS7008 transpile error", () => {
  const badSource = scriptTsSource.replace(
    "export function add(a: number, b: number): number { return a + b; }",
    "export function add(a: number, b: number): number { return a + }",
  );
  const compiled = compileSource(badSource, { target: "ts" });
  // The script body is opaque to the frontend, so this compiles cleanly.
  assert.deepEqual(compiled.diagnostics, [], JSON.stringify(compiled.diagnostics));
  const generated = generateTypeScript({
    moduleName: "app.main",
    ir: compiled.ir,
    sourceText: badSource,
  });
  const ns7008 = generated.diagnostics.filter((d) => d.code === "NS7008");
  assert.equal(ns7008.length, 1, JSON.stringify(generated.diagnostics));
  assert.match(ns7008[0].message, /failed to transpile/);
  assert.equal((generated.artifacts ?? []).length, 0);
});

test("the pinned TypeScript version matches the installed compiler exactly", () => {
  // A mismatch is a hard error, so the pin and the install must agree or the
  // build would refuse to run.
  assert.equal(pinnedTypeScriptVersionError(PINNED_TYPESCRIPT_VERSION), undefined);
  assert.match(pinnedTypeScriptVersionError("6.9.9"), /typescript@7\.0\.2 is required/);
  assert.match(pinnedTypeScriptVersionError("6.9.9"), /typescript@6\.9\.9 is installed/);

  const installed = JSON.parse(
    readFileSync(join(root, "node_modules", "typescript", "package.json"), "utf8"),
  );
  assert.equal(
    installed.version,
    PINNED_TYPESCRIPT_VERSION,
    "the installed typescript must match the pinned compiler version exactly",
  );
});
