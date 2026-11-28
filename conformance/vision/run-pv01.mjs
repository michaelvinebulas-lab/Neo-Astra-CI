// PV-01 proof driver: run one source through JS, TS, native, Nova, and the
// playground compiler bridge, then diff all stdout against a frozen expectation.
//
// Run from anywhere: node conformance/vision/run-pv01.mjs
// Overrides: ASTRA_CLI, NOVA_BIN, ASTRA_WASM.
import { spawnSync } from "node:child_process";
import {
  copyFileSync,
  existsSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { CompilerBridge } from "../../orchestrator/packages/playground/src/bridge.ts";
import { generateJavaScript } from "@neo-astra/codegen-js";

const HERE = dirname(fileURLToPath(import.meta.url));
const ROOT = resolve(HERE, "..", "..");
const ASTRA_CLI = process.env.ASTRA_CLI ?? join(ROOT, "orchestrator/packages/cli/dist/index.js");
const NOVA_BIN = process.env.NOVA_BIN ?? join(ROOT, "target/debug/nova.exe");
const TSC = join(ROOT, "node_modules/typescript/bin/tsc");
const WASM =
  process.env.ASTRA_WASM ??
  join(ROOT, "target/wasm32-unknown-unknown/debug/astra_wasm.wasm");
const REPORT = join(HERE, "pv01-report.md");

const normalize = (text) => text.replace(/\r\n/g, "\n").replace(/\s+$/, "");
const fenced = (text) => `\`\`\`text\n${normalize(text)}\n\`\`\``;

function run(executable, args, options = {}) {
  const result = spawnSync(executable, args, {
    cwd: options.cwd ?? ROOT,
    encoding: "utf8",
    maxBuffer: 16 * 1024 * 1024,
    windowsHide: true,
  });
  if (result.error) throw result.error;
  if (result.status !== 0 && !options.allowFailure) {
    throw new Error(
      `${executable} ${args.join(" ")} failed (${result.status}):\n${result.stderr ?? ""}\n${result.stdout ?? ""}`,
    );
  }
  return { stdout: result.stdout ?? "", stderr: result.stderr ?? "", status: result.status ?? 0 };
}

const sourceText = readFileSync(join(HERE, "pv01-contract.ns"), "utf8");
const expected = normalize(readFileSync(join(HERE, "pv01-contract.expected"), "utf8"));

// Build a throwaway project so the proof source and manifests cannot drift.
const project = mkdtempSync(join(tmpdir(), "pv01-"));
mkdirSync(join(project, "src"), { recursive: true });
writeFileSync(join(project, "src/main.ns"), sourceText);
copyFileSync(join(HERE, "pv01/core.json"), join(project, "core.json"));
copyFileSync(join(HERE, "pv01/build.json"), join(project, "build.json"));

const results = [];
let parity = { ok: false, detail: "not run" };

try {
  // The ESM artifacts export `main`; a tiny runner imports and calls it so the
  // captured stdout follows the conformance `RETURN=<code>` convention.
  const runner = (outDir) =>
    writeFileSync(
      join(project, outDir, "runner.mjs"),
      'import * as module from "./src/main.js";\n' +
        'const value = module.main();\n' +
        'process.stdout.write("RETURN=" + String(value));\n',
    );

  // JavaScript: build with the CLI, execute with Node.
  run(process.execPath, [ASTRA_CLI, "build", project, "--target", "js"]);
  runner("dist/js");
  const jsOut = run(process.execPath, [join(project, "dist/js/runner.mjs")]);
  results.push({
    target: "js",
    command: `astra build . --target js && node dist/js/runner.mjs`,
    stdout: jsOut.stdout,
  });

  // TypeScript: emit .ts, transpile in place with the pinned compiler, run.
  run(process.execPath, [ASTRA_CLI, "build", project, "--target", "ts"]);
  run(process.execPath, [
    TSC,
    "--ignoreConfig",
    "--noCheck",
    "--target",
    "es2022",
    "--module",
    "esnext",
    "--moduleResolution",
    "bundler",
    join(project, "dist/ts/src/main.ts"),
  ]);
  runner("dist/ts");
  const tsOut = run(process.execPath, [join(project, "dist/ts/runner.mjs")]);
  results.push({
    target: "ts",
    command: `astra build . --target ts && tsc --ignoreConfig --noCheck ... && node dist/ts/runner.mjs`,
    stdout: tsOut.stdout,
  });

  // Native: build with a hosted C toolchain and run the produced executable.
  run(process.execPath, [ASTRA_CLI, "build", project, "--target", "native"]);
  const exe = join(project, "dist/native", process.platform === "win32" ? "main.exe" : "main");
  if (!existsSync(exe)) throw new Error(`native build did not produce ${exe}`);
  const nativeOut = run(exe, []);
  results.push({
    target: "native",
    command: `astra build . --target native && ./dist/native/main`,
    stdout: `${nativeOut.stdout}RETURN=${nativeOut.status}`,
  });

  // Nova: interpret the canonical source directly.
  const novaOut = run(NOVA_BIN, ["run", join(project, "src/main.ns")]);
  results.push({
    target: "nova",
    command: `nova run src/main.ns`,
    stdout: `${novaOut.stdout}RETURN=${novaOut.status}`,
  });

  // Playground parity: compile through the Wasm bridge and generate JS with the
  // same backend the playground bundles; diff against the CLI's JS artifact.
  const cliJs = normalize(
    readFileSync(join(project, "dist/js/src/main.js"), "utf8").replace(
      /^\/\/# sourceMappingURL=.*$/m,
      "",
    ),
  );
  if (!existsSync(WASM)) {
    parity = { ok: false, detail: `wasm artifact missing: ${WASM}` };
  } else {
    const bridge = await CompilerBridge.instantiate(await readFileSync(WASM));
    try {
      const response = await bridge.request({
        jsonrpc: "2.0",
        id: 1,
        method: "compile",
        params: {
          apiVersion: "1.0",
          source: { path: "src/main.ns", text: sourceText },
          target: "js",
          integerPolicy: "wrapping",
        },
      });
      const generated = generateJavaScript({
        moduleName: "app.main",
        ir: response.ir,
        sourceText,
        backend: "js",
      });
      const playgroundJs = normalize(generated.code.replace(/^\/\/# sourceMappingURL=.*$/m, ""));
      parity = {
        ok: playgroundJs === cliJs,
        detail:
          playgroundJs === cliJs
            ? "playground bridge + codegen-js output matches dist/js/src/main.js"
            : "playground bridge + codegen-js output differs from dist/js/src/main.js",
        cliJs,
        playgroundJs,
      };
    } finally {
      bridge.close();
    }
  }
} finally {
  rmSync(project, { recursive: true, force: true });
}

const divergences = results.filter((result) => normalize(result.stdout) !== expected);
const allPass = divergences.length === 0 && parity.ok;

const lines = [
  "# PV-01 evidence report",
  "",
  `Generated: ${new Date().toISOString()}`,
  `Source: \`conformance/vision/pv01-contract.ns\`; expectation: \`conformance/vision/pv01-contract.expected\`.`,
  `Manifests: \`conformance/vision/pv01/{core,build}.json\` (edition 0.1, \`math.integers=wrapping\`, \`strings.model=utf8\`).`,
  "",
  "## Frozen expectation",
  "",
  fenced(expected),
  "",
  "## Captured stdout",
  "",
];
for (const result of results) {
  const status = normalize(result.stdout) === expected ? "PASS" : "FAIL";
  lines.push(`### ${result.target} — ${status}`, "", `\`${result.command}\``, "", fenced(result.stdout), "");
}
lines.push("## Playground worker parity", "", parity.ok ? "PASS" : "FAIL", "", parity.detail, "");
if (!parity.ok && parity.cliJs !== undefined) {
  lines.push("CLI JS head:", "", fenced(parity.cliJs.slice(0, 400)), "", "Playground JS head:", "", fenced(parity.playgroundJs.slice(0, 400)), "");
}
lines.push(
  "## Divergence classification",
  "",
  divergences.length === 0
    ? "No stdout divergences were observed."
    : divergences.map((d) => `- ${d.target}: classify as backend bug, missing policy plumbing, or spec ambiguity.`).join("\n"),
  "",
  `## Verdict`,
  "",
  allPass ? "PROVEN — all four targets and the playground bridge agree." : "PARTIAL — see the failures above.",
  "",
);
writeFileSync(REPORT, lines.join("\n"));
process.stdout.write(`${allPass ? "PV-01 PROVEN" : "PV-01 PARTIAL"} — report: ${REPORT}\n`);
if (!allPass) process.exitCode = 1;
