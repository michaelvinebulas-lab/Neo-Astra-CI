#!/usr/bin/env node

// Lockstep release bump. Every package moves together, so this script updates
// the root package, the Cargo workspace, every orchestrator package, the
// lockfile, and the two hard-coded CLI version constants, then moves the
// changelog's [Unreleased] section under the new dated version.
//
// Usage:
//   node scripts/release.mjs 1.0.0 --dry-run
//   node scripts/release.mjs 1.0.0
//
// The release checklist (Docs/release-checklist.md) still owns the commit,
// the Tier-1 matrix, packaging, and documentation. This script only performs
// the mechanical bump and refuses to leave a drift the gate would reject.

import { spawnSync } from "node:child_process";
import { readFileSync, readdirSync, writeFileSync } from "node:fs";
import { join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const defaultRoot = fileURLToPath(new URL("..", import.meta.url));
const rootOverride = process.argv.find((arg) => arg.startsWith("--root="));
const root = rootOverride
  ? resolve(rootOverride.slice("--root=".length))
  : defaultRoot;
const CHANGELOG = "CHANGELOG.md";
const LOCKFILE = "package-lock.json";
const CARGO = "Cargo.toml";
const CARGO_LOCK = "Cargo.lock";
const PACKAGES = "orchestrator/packages";
const CLI_CONSTANTS = [
  "orchestrator/packages/cli/src/index.ts",
  "orchestrator/packages/nopm/src/cli.ts",
];

function fail(message) {
  process.stderr.write(`[release] ${message}\n`);
  process.exit(1);
}

function parseVersion(text) {
  const match = /^(\d+)\.(\d+)\.(\d+)(?:-([0-9A-Za-z.-]+))?$/.exec(text);
  if (!match) {
    fail(`'${text}' is not a version (expected major.minor.patch).`);
  }
  return {
    major: Number(match[1]),
    minor: Number(match[2]),
    patch: Number(match[3]),
    pre: match[4] ?? null,
  };
}

function compareVersions(left, right) {
  for (const key of ["major", "minor", "patch"]) {
    if (left[key] !== right[key]) {
      return left[key] < right[key] ? -1 : 1;
    }
  }
  if (left.pre === right.pre) {
    return 0;
  }
  // A pre-release sorts before the release it leads to.
  if (left.pre === null) {
    return 1;
  }
  if (right.pre === null) {
    return -1;
  }
  return left.pre < right.pre ? -1 : 1;
}

function read(path) {
  return readFileSync(join(root, path), "utf8");
}

function readJson(path) {
  try {
    return JSON.parse(read(path));
  } catch (error) {
    fail(`${path}: ${error instanceof Error ? error.message : String(error)}`);
  }
}

function readCurrentVersion() {
  const manifest = readJson("package.json");
  if (typeof manifest.version !== "string") {
    fail("package.json has no version.");
  }
  return manifest.version;
}

function updateJsonVersion(path, transform) {
  const value = readJson(path);
  transform(value);
  writeFileSync(join(root, path), `${JSON.stringify(value, null, 2)}\n`, "utf8");
}

function updateText(path, pattern, replacement) {
  const source = read(path);
  if (!pattern.test(source)) {
    fail(`${path}: expected pattern not found.`);
  }
  writeFileSync(join(root, path), source.replace(pattern, replacement), "utf8");
}

const args = process.argv.slice(2);
const dryRun = args.includes("--dry-run");
const skipCargoLock = args.includes("--skip-cargo-lock");
const versionArg = args.find((arg) => !arg.startsWith("--"));
if (!versionArg) {
  fail("usage: node scripts/release.mjs <version> [--dry-run]");
}
const next = parseVersion(versionArg);
const currentText = readCurrentVersion();
const current = parseVersion(currentText);
if (compareVersions(next, current) <= 0) {
  fail(`new version ${versionArg} must be greater than ${currentText}.`);
}

const date = new Date().toISOString().slice(0, 10);
const changelog = read(CHANGELOG);
const unreleasedHeading = /^## \[Unreleased\][ \t]*$/m.exec(changelog);
if (!unreleasedHeading) {
  fail(`${CHANGELOG}: no [Unreleased] section.`);
}
const bodyStart = unreleasedHeading.index + unreleasedHeading[0].length;
const nextHeading = /^## \[/m.exec(changelog.slice(bodyStart));
const bodyEnd = nextHeading ? bodyStart + nextHeading.index : changelog.length;
const body = changelog.slice(bodyStart, bodyEnd).trim();
if (body.length === 0) {
  fail(`${CHANGELOG}: [Unreleased] has no entries.`);
}
const before = changelog.slice(0, unreleasedHeading.index);
const after = nextHeading ? changelog.slice(bodyEnd) : "";
const nextChangelog = `${before}## [Unreleased]\n\n## [${versionArg}] - ${date}\n\n${body}\n\n${after}`.trimEnd() + "\n";

const planned = [
  "package.json",
  CARGO,
  LOCKFILE,
  CHANGELOG,
  ...CLI_CONSTANTS,
];
for (const entry of readdirSync(join(root, PACKAGES), { withFileTypes: true })) {
  if (entry.isDirectory()) {
    planned.push(`${PACKAGES}/${entry.name}/package.json`);
  }
}

if (dryRun) {
  process.stdout.write(
    `[release] dry run: ${currentText} -> ${versionArg} (${date})\n`,
  );
  for (const path of planned) {
    process.stdout.write(`  would update ${path}\n`);
  }
  process.stdout.write(
    `  would move ${CHANGELOG} [Unreleased] under [${versionArg}]\n`,
  );
  process.exit(0);
}

updateJsonVersion("package.json", (manifest) => {
  manifest.version = versionArg;
});

updateText(
  CARGO,
  /(^\[workspace\.package\]\s*\n[\s\S]*?^version\s*=\s*")[^"]+(")/m,
  `$1${versionArg}$2`,
);

for (const entry of readdirSync(join(root, PACKAGES), { withFileTypes: true })) {
  if (!entry.isDirectory()) {
    continue;
  }
  const path = `${PACKAGES}/${entry.name}/package.json`;
  const manifest = readJson(path);
  manifest.version = versionArg;
  for (const section of [
    "dependencies",
    "devDependencies",
    "peerDependencies",
    "optionalDependencies",
  ]) {
    const dependencies = manifest[section];
    if (dependencies === null || typeof dependencies !== "object") {
      continue;
    }
    for (const name of Object.keys(dependencies)) {
      if (name.startsWith("@neo-astra/")) {
        dependencies[name] = versionArg;
      }
    }
  }
  writeFileSync(join(root, path), `${JSON.stringify(manifest, null, 2)}\n`, "utf8");
}

for (const path of CLI_CONSTANTS) {
  updateText(
    path,
    /^(?:export\s+)?const VERSION\s*=\s*"[^"]+";/m,
    `const VERSION = "${versionArg}";`,
  );
}

updateJsonVersion(LOCKFILE, (lockfile) => {
  lockfile.version = versionArg;
  if (lockfile.packages?.[""]) {
    lockfile.packages[""].version = versionArg;
  }
  for (const [path, entry] of Object.entries(lockfile.packages ?? {})) {
    if (path.startsWith(`${PACKAGES}/`)) {
      entry.version = versionArg;
    }
  }
});

writeFileSync(join(root, CHANGELOG), nextChangelog, "utf8");

// Cargo rewrites the lockfile when the workspace members' versions change.
if (!skipCargoLock) {
  const metadata = spawnSync(
    "cargo",
    ["metadata", "--format-version", "1"],
    { cwd: root, encoding: "utf8", stdio: ["ignore", "ignore", "pipe"] },
  );
  if (metadata.error || metadata.status !== 0) {
    fail(
      `cargo metadata failed; ${CARGO_LOCK} may be stale:\n${metadata.stderr ?? ""}`,
    );
  }
}

const check = spawnSync(
  process.execPath,
  [resolve(defaultRoot, "scripts/check-versions.mjs"), "--root", root],
  { cwd: root, encoding: "utf8", stdio: ["ignore", "pipe", "pipe"] },
);
process.stdout.write(check.stdout ?? "");
if (check.status !== 0) {
  process.stderr.write(check.stderr ?? "");
  fail("version check failed after the bump.");
}

process.stdout.write(
  `[release] bumped ${planned.length} files from ${currentText} to ${versionArg} (${date})\n`,
);
process.stdout.write(
  `[release] next: review the changelog, run npm run ci, commit, and poll Tier-1.\n`,
);
