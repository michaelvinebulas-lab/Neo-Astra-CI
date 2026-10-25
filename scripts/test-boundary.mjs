#!/usr/bin/env node

import { spawnSync } from "node:child_process";
import { existsSync } from "node:fs";
import { homedir } from "node:os";
import { delimiter, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const root = resolve(fileURLToPath(new URL("..", import.meta.url)));
const cargoBin = resolve(homedir(), ".cargo", "bin");
const env = {
  ...process.env,
  PATH: `${cargoBin}${delimiter}${process.env.PATH ?? ""}`,
};

function run(command, args) {
  process.stdout.write(`[boundary] ${[command, ...args].join(" ")}\n`);
  const result = spawnSync(command, args, {
    cwd: root,
    env,
    stdio: "inherit",
  });

  if (result.error) {
    process.stderr.write(`[boundary] ${result.error.message}\n`);
    process.exit(1);
  }

  if (result.status !== 0) {
    process.exit(result.status ?? 1);
  }
}

const cli = resolve(root, "orchestrator", "packages", "cli", "dist", "index.js");
const compilerClient = resolve(
  root,
  "orchestrator",
  "packages",
  "compiler-client",
  "dist",
  "index.js",
);
const codegenJs = resolve(
  root,
  "orchestrator",
  "packages",
  "codegen-js",
  "dist",
  "index.js",
);
if (!existsSync(cli) || !existsSync(compilerClient) || !existsSync(codegenJs)) {
  if (process.platform === "win32") {
    run(process.env.ComSpec ?? "cmd.exe", ["/d", "/s", "/c", "npm run build"]);
  } else {
    run("npm", ["run", "build"]);
  }
}

run("cargo", ["build", "-p", "astra-compiler-api", "--bin", "astra-compiler"]);
run("cargo", ["build", "-p", "astra-wasm", "--target", "wasm32-unknown-unknown"]);
run(process.execPath, [
  "--test",
  "orchestrator/tests/compiler-api.test.mjs",
  "orchestrator/tests/format-cli.test.mjs",
  "orchestrator/tests/codegen-js.test.mjs",
]);
