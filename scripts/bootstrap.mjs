#!/usr/bin/env node

import { spawnSync } from "node:child_process";
import { existsSync } from "node:fs";
import { homedir, platform, arch } from "node:os";
import { delimiter, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const root = resolve(fileURLToPath(new URL("..", import.meta.url)));
const isWindows = platform() === "win32";
const isMac = platform() === "darwin";
const isLinux = platform() === "linux";
const checkOnly = process.argv.includes("--check");
const verify = process.argv.includes("--verify");
const skipSystem = process.argv.includes("--skip-system");
const cargoBin = join(homedir(), ".cargo", "bin");
const env = {
  ...process.env,
  PATH: `${cargoBin}${delimiter}${process.env.PATH ?? ""}`,
};

const results = [];

function log(message) {
  process.stdout.write(`${message}\n`);
}

function spawnCrossPlatform(command, args, options = {}) {
  const { shell = isWindows, ...spawnOptions } = options;

  if (isWindows && shell) {
    const line = [command, ...args].join(" ");
    return spawnSync(process.env.ComSpec ?? "cmd.exe", ["/d", "/s", "/c", line], {
      ...spawnOptions,
      shell: false,
    });
  }

  return spawnSync(command, args, { ...spawnOptions, shell: false });
}

function run(command, args, options = {}) {
  const label = [command, ...args].join(" ");
  if (checkOnly) {
    log(`[check] ${label}`);
    return { ok: true, status: 0 };
  }

  log(`[run] ${label}`);
  const result = spawnCrossPlatform(command, args, {
    cwd: options.cwd ?? root,
    env: options.env ?? env,
    stdio: options.stdio ?? "inherit",
    shell: options.shell,
    timeout: options.timeout ?? 900_000,
  });

  if (result.error) {
    log(`[warn] ${label}: ${result.error.message}`);
    return { ok: false, status: 1 };
  }

  if (result.status !== 0 && !options.optional) {
    log(`[fail] ${label} exited with ${result.status ?? "unknown"}`);
  }

  return { ok: result.status === 0, status: result.status ?? 1 };
}

function capture(command, args) {
  const result = spawnCrossPlatform(command, args, {
    cwd: root,
    env,
    encoding: "utf8",
  });
  return result.status === 0 ? (result.stdout ?? "").trim() : "";
}

function which(command) {
  return capture(isWindows ? "where" : "which", [command]);
}

function has(command) {
  return which(command).length > 0;
}

function record(name, ok, detail) {
  results.push({ name, ok, detail });
  log(`${ok ? "[ok]" : "[missing]"} ${name}${detail ? ` — ${detail}` : ""}`);
}

const nopmCli = join(root, "orchestrator", "packages", "nopm", "dist", "cli.js");

function ensureNopmBuilt() {
  if (existsSync(nopmCli)) {
    return true;
  }
  // The system source lives in the TypeScript workspace, so the CLI must be
  // built before bootstrap can delegate to it. tsc is available after
  // `npm install`, which runs earlier in a real bootstrap.
  const result = run(isWindows ? "npm.cmd" : "npm", ["run", "build"], { optional: false });
  return result.ok && existsSync(nopmCli);
}

function systemTool(name, tool, optional = false) {
  if (has(name)) {
    record(name, true, which(name).split(/\r?\n/)[0]);
    return true;
  }

  if (skipSystem) {
    record(name, false, "missing; skipped because --skip-system was used");
    return false;
  }

  if (checkOnly) {
    record(name, false, "not installed");
    return false;
  }

  if (!ensureNopmBuilt()) {
    record(name, false, "nopm is not built; run npm run bootstrap");
    return false;
  }

  log(`[run] nopm add sys:${tool}`);
  const result = spawnSync(process.execPath, [nopmCli, "add", `sys:${tool}`], {
    cwd: root,
    env,
    stdio: "inherit",
    timeout: 900_000,
  });
  if (result.error || result.status !== 0) {
    record(name, false, optional ? "install failed or needs elevation" : "install failed");
    return false;
  }

  record(name, true, "installed with the system package manager");
  return true;
}

function installRustup() {
  if (has("rustup")) {
    record("rustup", true, which("rustup").split(/\r?\n/)[0]);
    return true;
  }

  if (checkOnly) {
    record("rustup", false, "not installed");
    return false;
  }

  if (isWindows) {
    if (!has("winget")) {
      record("rustup", false, "winget is unavailable");
      return false;
    }
    const result = run(
      "winget",
      [
        "install",
        "--id",
        "Rustlang.Rustup",
        "-e",
        "--accept-package-agreements",
        "--accept-source-agreements",
        "--silent",
      ],
      { timeout: 900_000 },
    );
    if (!result.ok) {
      record("rustup", false, "install failed or needs elevation");
      return false;
    }
    record("rustup", true, "installed with winget");
    return true;
  }
  if (isMac) {
    if (!has("brew")) {
      record("rustup", false, "Homebrew is unavailable");
      return false;
    }
    const result = run("brew", ["install", "rustup"], { timeout: 900_000 });
    if (!result.ok) {
      record("rustup", false, "install failed");
      return false;
    }
    record("rustup", true, "installed with Homebrew");
    return true;
  }

  if (!has("curl")) {
    record("rustup", false, "curl is unavailable");
    return false;
  }

  const result = run(
    "sh",
    ["-c", "curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal"],
    { optional: false },
  );
  if (!result.ok) {
    record("rustup", false, "rustup installation failed");
    return false;
  }

  record("rustup", true, "installed with rustup.rs");
  return true;
}

function msvcLinkerAvailable() {
  if (!isWindows) {
    return false;
  }
  if (has("link")) {
    return true;
  }

  const vswhere = "C:\\Program Files (x86)\\Microsoft Visual Studio\\Installer\\vswhere.exe";
  if (!existsSync(vswhere)) {
    return false;
  }

  const result = spawnSync(
    vswhere,
    [
      "-latest",
      "-products",
      "*",
      "-requires",
      "Microsoft.VisualStudio.Component.VC.Tools.x86.x64",
      "-property",
      "installationPath",
    ],
    { encoding: "utf8" },
  );

  return result.status === 0 && (result.stdout ?? "").trim().length > 0;
}

function rustToolchain() {
  if (isWindows && !msvcLinkerAvailable()) {
    return "stable-x86_64-pc-windows-gnu";
  }
  return "stable";
}

function main() {
  log(`Neo Astra bootstrap — ${platform()} ${arch()}`);
  log(`Workspace: ${root}`);

  const nodeMajor = Number(process.versions.node.split(".")[0]);
  record("node", nodeMajor >= 22, process.version);
  record("npm", has("npm"), has("npm") ? capture(isWindows ? "npm.cmd" : "npm", ["--version"]) : "missing");

  if (nodeMajor < 22 || !has("npm")) {
    log("\nNode.js 22+ and npm are required before running this bootstrap.");
    process.exitCode = 1;
    return;
  }

  if (!checkOnly) {
    const npm = isWindows ? "npm.cmd" : "npm";
    const install = run(npm, ["install"], { optional: false });
    record("JavaScript dependencies", install.ok, install.ok ? "npm install complete" : "npm install failed");
  }

  systemTool("git", "git");
  systemTool("cmake", "cmake");
  systemTool("clang", "clang", true);

  if (!installRustup()) {
    process.exitCode = 1;
    return;
  }

  const toolchain = rustToolchain();
  record("Rust toolchain", true, toolchain);
  if (!checkOnly) {
    run("rustup", ["toolchain", "install", toolchain, "--profile", "minimal"], { optional: false });
    run("rustup", ["default", toolchain], { optional: false });
    run("rustup", ["component", "add", "rustfmt", "clippy", "--toolchain", toolchain], {
      optional: false,
    });
    run(
      "rustup",
      ["target", "add", "wasm32-unknown-unknown", "wasm32-wasip1", "--toolchain", toolchain],
      { optional: true },
    );
    run("cargo", ["fetch", "--workspace"], { optional: false });
  }

  const cargo = capture("cargo", ["--version"]);
  const rustc = capture("rustc", ["--version"]);
  record("cargo", cargo.length > 0, cargo || "missing");
  record("rustc", rustc.length > 0, rustc || "missing");
  record("rustfmt", capture("cargo", ["fmt", "--version"]).length > 0, capture("cargo", ["fmt", "--version"]));
  record("clippy", capture("cargo", ["clippy", "--version"]).length > 0, capture("cargo", ["clippy", "--version"]));

  if (verify && !checkOnly) {
    run(isWindows ? "npm.cmd" : "npm", ["run", "ci"], { optional: false });
  }

  const required = results.filter((entry) =>
    ["node", "npm", "git", "cmake", "rustup", "cargo", "rustc", "rustfmt", "clippy"].includes(entry.name),
  );
  const missingRequired = required.filter((entry) => !entry.ok);

  log("\nSummary");
  for (const entry of results) {
    log(`- ${entry.ok ? "ok" : "missing"}: ${entry.name}${entry.detail ? ` (${entry.detail})` : ""}`);
  }

  if (missingRequired.length > 0) {
    log(`\n${missingRequired.length} required tool(s) are still missing.`);
    process.exitCode = 1;
  } else {
    log("\nBootstrap complete.");
  }
}

main();
