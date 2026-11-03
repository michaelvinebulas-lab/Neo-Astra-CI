#!/usr/bin/env node

import { spawnSync } from "node:child_process";
import { homedir } from "node:os";
import { delimiter, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const root = resolve(fileURLToPath(new URL("..", import.meta.url)));
const cargoBin = resolve(homedir(), ".cargo", "bin");
const env = {
  ...process.env,
  PATH: `${cargoBin}${delimiter}${process.env.PATH ?? ""}`,
};
const npm = process.platform === "win32" ? "npm.cmd" : "npm";

function run(label, command, args, extraEnv = {}) {
  process.stdout.write(`\n[ci] ${label}\n`);
  const runEnv = { ...env, ...extraEnv };
  const result =
    process.platform === "win32"
      ? spawnSync(
          process.env.ComSpec ?? "cmd.exe",
          ["/d", "/s", "/c", [command, ...args].join(" ")],
          { cwd: root, env: runEnv, stdio: "inherit", shell: false },
        )
      : spawnSync(command, args, {
          cwd: root,
          env: runEnv,
          stdio: "inherit",
          shell: false,
        });

  if (result.error) {
    process.stderr.write(`[ci] ${result.error.message}\n`);
    process.exit(1);
  }

  if (result.status !== 0) {
    process.exit(result.status ?? 1);
  }
}

run("Version check", "node", ["scripts/check-versions.mjs"]);
run("TypeScript build", npm, ["run", "build"]);
run("TypeScript tests", npm, ["test"]);
run("Native/Wasm boundary", npm, ["run", "test:boundary"]);
run("Rust format", "cargo", ["fmt", "--all", "--check"]);
run("Rust clippy", "cargo", ["clippy", "--workspace", "--all-targets", "--", "-D", "warnings"]);
run("Rust tests", "cargo", ["test", "--workspace"]);
// GC-001 §8.1: run the hosted C tests again with collection at every managed
// allocation and at every safepoint, so a missing or stale root slot fails the
// gate instead of hiding until collection is on by default.
run(
  "Rust tests (GC stress)",
  "cargo",
  [
    "test",
    "-p",
    "astra-codegen-c",
    "--test",
    "emit_c",
    "--test",
    "conformance",
  ],
  { ASTRA_GC_STRESS: "1" },
);

process.stdout.write("\n[ci] all checks passed\n");
