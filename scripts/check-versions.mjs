#!/usr/bin/env node

// Lockstep version check for the gate. The release policy (Docs/state.md)
// keeps the root package, the Cargo workspace, and every orchestrator package
// on one version. This script fails fast when one of them drifts, before the
// build and tests spend time on a tree that cannot be released.

import { readFileSync, readdirSync } from "node:fs";
import { join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const defaultRoot = fileURLToPath(new URL("..", import.meta.url));
const rootOverride = process.argv.find((arg) => arg.startsWith("--root="));
const root = rootOverride
  ? resolve(rootOverride.slice("--root=".length))
  : defaultRoot;
const SEMVER = /^\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?(?:\+[0-9A-Za-z.-]+)?$/;
const errors = [];
const checked = [];

function read(path) {
  return readFileSync(join(root, path), "utf8");
}

function readJson(path) {
  try {
    return JSON.parse(read(path));
  } catch (error) {
    errors.push(`${path}: ${error instanceof Error ? error.message : String(error)}`);
    return null;
  }
}

function expect(path, value, expected) {
  checked.push(path);
  if (typeof value !== "string" || value.length === 0) {
    errors.push(`${path}: missing version`);
    return;
  }
  if (value !== expected) {
    errors.push(`${path}: ${value}, expected ${expected}`);
  }
}

const rootPackage = readJson("package.json");
const expected = rootPackage?.version;
if (typeof expected !== "string" || !SEMVER.test(expected)) {
  errors.push(`package.json: invalid version '${String(expected)}'`);
}

const cargo = read("Cargo.toml");
const workspacePackage = cargo.match(
  /^\[workspace\.package\]\s*\n([\s\S]*?)(?=\n\[|$)/m,
);
const cargoVersion = workspacePackage?.[1].match(/^version\s*=\s*"([^"]+)"/m)?.[1];
expect("Cargo.toml [workspace.package]", cargoVersion, expected);

const lock = readJson("package-lock.json");
expect("package-lock.json", lock?.version, expected);

const packagesDir = "orchestrator/packages";
for (const entry of readdirSync(join(root, packagesDir), { withFileTypes: true })) {
  if (!entry.isDirectory()) {
    continue;
  }
  const packagePath = `${packagesDir}/${entry.name}/package.json`;
  const manifest = readJson(packagePath);
  if (manifest === null) {
    continue;
  }
  expect(packagePath, manifest.version, expected);
}

// The CLIs print their version without importing package.json.
for (const sourcePath of [
  "orchestrator/packages/cli/src/index.ts",
  "orchestrator/packages/nopm/src/cli.ts",
]) {
  const match = read(sourcePath).match(
    /^(?:export\s+)?const VERSION\s*=\s*"([^"]+)";/m,
  );
  expect(`${sourcePath} const VERSION`, match?.[1], expected);
}

if (errors.length > 0) {
  process.stderr.write("[versions] lockstep version check failed:\n");
  for (const error of errors) {
    process.stderr.write(`  - ${error}\n`);
  }
  process.exit(1);
}

process.stdout.write(
  `[versions] all packages at ${expected} (${checked.length} files)\n`,
);
