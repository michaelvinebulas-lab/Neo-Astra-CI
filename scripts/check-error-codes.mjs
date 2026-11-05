#!/usr/bin/env node

// Keeps the normative diagnostic tables and the implementation in sync.
//
// Every code that appears in compiler/CLI source must have a row in
// Docs/spec-v0.1.md. Every code in the spec must either appear in source or be
// listed here with the reason it is reserved. Run with --report to inspect
// drift without failing the build.

import { readdirSync, readFileSync, statSync } from "node:fs";
import { join, relative, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const root = resolve(fileURLToPath(new URL("..", import.meta.url)));
const reportOnly = process.argv.includes("--report");
const codePattern = /\b(?:NS|NP|NR|NV|PG)\d{4}\b/g;
const specPath = join(root, "Docs", "spec-v0.1.md");

// Codes that are deliberately not emitted in v0.1. Keep this list short and
// give each entry a reason; a reserved code is still part of the stable range.
const reserved = new Map([
  ["NS0001", "Test fixture used by the diagnostic and compiler-API unit tests."],
  ["NS1004", "Reserved: invalid UTF-8 source is reported by the CLI as NS9007."],
  ["NS2007", "Reserved: no parser path emits an invalid-modifier diagnostic in v0.1."],
  ["NS4008", "Reserved by the type-diagnostic table."],
  ["NS9999", "Reserved sentinel; tests assert it never appears in output."],
]);

const skipDirectories = new Set(["node_modules", "target", "dist", ".git", "tests"]);
const sourceRoots = ["crates", "orchestrator/packages", "scripts"];

function walk(directory, files = []) {
  for (const entry of readdirSync(directory)) {
    if (skipDirectories.has(entry)) {
      continue;
    }
    const path = join(directory, entry);
    const stat = statSync(path);
    if (stat.isDirectory()) {
      walk(path, files);
    } else if (/\.(rs|ts|mjs|js)$/.test(entry)) {
      files.push(path);
    }
  }
  return files;
}

function codesIn(text) {
  return new Set(text.match(codePattern) ?? []);
}

const emitted = new Map();
for (const sourceRoot of sourceRoots) {
  const absolute = join(root, sourceRoot);
  let files;
  try {
    files = walk(absolute);
  } catch {
    continue;
  }
  for (const file of files) {
    if (relative(root, file) === "scripts/check-error-codes.mjs") {
      continue;
    }
    for (const code of codesIn(readFileSync(file, "utf8"))) {
      if (!emitted.has(code)) {
        emitted.set(code, new Set());
      }
      emitted.get(code).add(relative(root, file));
    }
  }
}

const spec = readFileSync(specPath, "utf8");
const specCodes = new Set();
const rowPattern = /^\|\s*`((?:NS|NP|NR|NV|PG)\d{4})`\s*\|/gm;
let match;
while ((match = rowPattern.exec(spec)) !== null) {
  specCodes.add(match[1]);
}

const missingFromSpec = [...emitted.keys()]
  .filter((code) => !specCodes.has(code) && !reserved.has(code))
  .sort();
const missingFromCode = [...specCodes]
  .filter((code) => !emitted.has(code) && !reserved.has(code))
  .sort();

process.stdout.write(
  `[codes] emitted=${emitted.size} spec=${specCodes.size} reserved=${reserved.size}\n`,
);

if (missingFromSpec.length > 0) {
  process.stdout.write("\nEmitted but absent from Docs/spec-v0.1.md:\n");
  for (const code of missingFromSpec) {
    process.stdout.write(`  ${code}  ${[...emitted.get(code)].slice(0, 3).join(", ")}\n`);
  }
}

if (missingFromCode.length > 0) {
  process.stdout.write("\nDocumented but not emitted (add to the reserved list or remove):\n");
  for (const code of missingFromCode) {
    process.stdout.write(`  ${code}\n`);
  }
}

if (missingFromSpec.length > 0 || missingFromCode.length > 0) {
  if (!reportOnly) {
    process.exitCode = 1;
  }
}
