#!/usr/bin/env node

import { spawnSync } from "node:child_process";
import {
  existsSync,
  mkdirSync,
  readFileSync,
  readdirSync,
  rmSync,
  statSync,
  writeFileSync,
} from "node:fs";
import { dirname, join, relative } from "node:path";
import { fileURLToPath } from "node:url";

const root = fileURLToPath(new URL("..", import.meta.url));
const args = process.argv.slice(2);
const out = args.includes("--out")
  ? args[args.indexOf("--out") + 1]
  : join(root, "dist", "public-source");
const keepGit = args.includes("--keep-git");

const skippedDirectories = new Set([
  ".git",
  ".tools",
  "Docs",
  "conformance",
  "dist",
  "fuzz",
  "node_modules",
  "playground-app",
  "public",
  "target",
  "test_comparison",
  "tools",
]);
const skippedFiles = new Set([
  ".github/workflows/ci.yml",
  "CHANGELOG.md",
  "README.md",
  "context.md",
  "crates/README.md",
  "orchestrator/README.md",
  "nvdia.ps1",
  "scripts/bootstrap.mjs",
  "scripts/public-source.mjs",
  "scripts/README.md",
  "scripts/npm/README.md",
  "scripts/native-parity.mjs",
  "scripts/check-error-codes.mjs",
  "scripts/check-versions.mjs",
  "scripts/ci.mjs",
  "scripts/release.mjs",
  "scripts/test-boundary.mjs",
]);
const skippedPrefixes = [
  "crates/",
  "orchestrator/packages/",
];

function isSkipped(relativePath) {
  const normalized = relativePath.replaceAll("\\", "/");
  if (skippedFiles.has(normalized)) {
    return true;
  }
  if (normalized.endsWith("/tests.rs") || normalized.endsWith("/dev/smoke.mjs")) {
    return true;
  }
  if (normalized.includes("/tests/") || normalized.endsWith("/tests")) {
    return true;
  }
  if (normalized.endsWith("/dist") || normalized.endsWith("/node_modules")) {
    return true;
  }
  if (normalized.includes("/dist/") || normalized.endsWith(".tsbuildinfo")) {
    return true;
  }
  if (normalized.startsWith("crates/") || normalized.startsWith("orchestrator/packages/")) {
    return false;
  }
  return skippedDirectories.has(normalized) || skippedDirectories.has(normalized.split("/")[0]);
}

function copyTree(source, destination, relativePath = "") {
  for (const entry of readdirSync(source, { withFileTypes: true })) {
    const childRelative = relativePath === "" ? entry.name : `${relativePath}/${entry.name}`;
    if (isSkipped(childRelative)) {
      continue;
    }
    const childSource = join(source, entry.name);
    const childDestination = join(destination, entry.name);
    if (entry.isDirectory()) {
      mkdirSync(childDestination, { recursive: true });
      copyTree(childSource, childDestination, childRelative);
    } else if (entry.isFile()) {
      const text = readFileSync(childSource, "utf8");
      const cleaned = cleanFile(childRelative, text);
      mkdirSync(dirname(childDestination), { recursive: true });
      writeFileSync(childDestination, cleaned, "utf8");
    }
  }
}

function cleanFile(path, text) {
  const normalized = path.replaceAll("\\", "/");
  let cleaned;
  if (normalized.endsWith(".rs")) {
    const stripped = stripRust(removeRustTests(text));
    cleaned =
      normalized === "crates/astra-codegen-c/src/runtime.rs"
        ? stripCCommentsInRawStrings(stripped)
        : stripped;
  } else if (normalized.endsWith(".ts") || normalized.endsWith(".mjs") || normalized.endsWith(".js")) {
    cleaned = stripJs(text);
  } else if (normalized.endsWith(".ns")) {
    cleaned = stripJs(text);
  } else if (normalized.endsWith(".toml")) {
    cleaned = stripHashComments(text);
  } else if (normalized.endsWith(".json")) {
    cleaned = stripJs(text);
  } else if (normalized.endsWith(".yml") || normalized.endsWith(".yaml")) {
    cleaned = stripHashComments(text);
  } else if (normalized === ".gitignore") {
    cleaned = stripHashComments(text);
  } else {
    cleaned = text;
  }
  return tidyText(cleaned);
}

function tidyText(text) {
  return text
    .replace(/\r\n/g, "\n")
    .replace(/[ \t]+\n/g, "\n")
    .replace(/\n{3,}/g, "\n\n")
    .replace(/^\n+/, "")
    .replace(/\n+$/, "\n");
}

function removeRustTests(text) {
  let result = text.replace(/#\[cfg\(test\)\]\s*mod\s+tests\s*;/g, "");
  const marker = "#[cfg(test)]";
  const index = result.indexOf(marker);
  if (index >= 0) {
    result = `${result.slice(0, index).replace(/\s+$/, "")}\n`;
  }
  return result;
}

function stripHashComments(text) {
  let out = "";
  let i = 0;
  while (i < text.length) {
    const c = text[i];
    if (c === '"' || c === "'") {
      const quote = c;
      out += c;
      i++;
      while (i < text.length) {
        const current = text[i];
        out += current;
        i++;
        if (current === "\\" && i < text.length) {
          out += text[i];
          i++;
          continue;
        }
        if (current === quote) {
          break;
        }
      }
      continue;
    }
    if (c === "#") {
      while (i < text.length && text[i] !== "\n") {
        i++;
      }
      continue;
    }
    out += c;
    i++;
  }
  return out;
}

function stripJs(text) {
  let out = "";
  let i = 0;
  let regexAllowed = true;
  while (i < text.length) {
    const c = text[i];
    if (c === '"' || c === "'") {
      const end = readQuoted(text, i, c);
      out += text.slice(i, end);
      i = end;
      regexAllowed = false;
      continue;
    }
    if (c === "`") {
      const end = readTemplate(text, i);
      out += text.slice(i, end);
      i = end;
      regexAllowed = false;
      continue;
    }
    if (c === "/" && text[i + 1] === "/") {
      i += 2;
      while (i < text.length && text[i] !== "\n") {
        i++;
      }
      continue;
    }
    if (c === "/" && text[i + 1] === "*") {
      i += 2;
      out += " ";
      while (i < text.length) {
        if (text[i] === "*" && text[i + 1] === "/") {
          i += 2;
          break;
        }
        if (text[i] === "\n") {
          out += "\n";
        }
        i++;
      }
      continue;
    }
    if (c === "/" && regexAllowed) {
      const end = readRegex(text, i);
      if (end !== undefined) {
        out += text.slice(i, end);
        i = end;
        regexAllowed = false;
        continue;
      }
    }
    out += c;
    if (/\s/.test(c)) {
      // Whitespace does not change whether a slash starts a regex.
    } else if (/[A-Za-z0-9_$]/.test(c)) {
      regexAllowed = false;
    } else if (c === ")" || c === "]" || c === "}") {
      regexAllowed = false;
    } else {
      regexAllowed = true;
    }
    i++;
  }
  return out;
}

function readQuoted(text, start, quote) {
  let i = start + 1;
  while (i < text.length) {
    if (text[i] === "\\") {
      i += 2;
      continue;
    }
    if (text[i] === quote) {
      return i + 1;
    }
    i++;
  }
  return text.length;
}

function readTemplate(text, start) {
  let i = start + 1;
  while (i < text.length) {
    if (text[i] === "\\") {
      i += 2;
      continue;
    }
    if (text[i] === "`") {
      return i + 1;
    }
    i++;
  }
  return text.length;
}

function readRegex(text, start) {
  let i = start + 1;
  let inClass = false;
  while (i < text.length) {
    const c = text[i];
    if (c === "\\") {
      i += 2;
      continue;
    }
    if (c === "\n") {
      return undefined;
    }
    if (c === "[") {
      inClass = true;
    } else if (c === "]") {
      inClass = false;
    } else if (c === "/" && !inClass) {
      i++;
      while (i < text.length && /[A-Za-z]/.test(text[i])) {
        i++;
      }
      return i;
    }
    i++;
  }
  return undefined;
}

function stripRust(text) {
  let out = "";
  let i = 0;
  while (i < text.length) {
    const c = text[i];
    if (c === "/" && text[i + 1] === "/") {
      i += 2;
      while (i < text.length && text[i] !== "\n") {
        i++;
      }
      continue;
    }
    if (c === "/" && text[i + 1] === "*") {
      i += 2;
      let depth = 1;
      out += " ";
      while (i < text.length && depth > 0) {
        if (text[i] === "/" && text[i + 1] === "*") {
          depth++;
          i += 2;
          continue;
        }
        if (text[i] === "*" && text[i + 1] === "/") {
          depth--;
          i += 2;
          continue;
        }
        if (text[i] === "\n") {
          out += "\n";
        }
        i++;
      }
      continue;
    }
    if (c === "r" || c === "b" || c === "c") {
      const raw = /^(?:b|c)?r(#*)"/.exec(text.slice(i));
      if (raw !== null) {
        const prefixLength = raw[0].length;
        const hashes = raw[1];
        const terminator = `"${hashes}`;
        const close = text.indexOf(terminator, i + prefixLength);
        const end = close < 0 ? text.length : close + terminator.length;
        out += text.slice(i, end);
        i = end;
        continue;
      }
    }
    if (c === '"') {
      const end = readQuoted(text, i, c);
      out += text.slice(i, end);
      i = end;
      continue;
    }
    if (c === "'") {
      const next = text[i + 1];
      const nextNext = text[i + 2];
      if (next === "\\" || nextNext === "'") {
        const end = readQuoted(text, i, c);
        out += text.slice(i, end);
        i = end;
        continue;
      }
      out += c;
      i++;
      continue;
    }
    out += c;
    i++;
  }
  return out;
}

function stripCCommentsInRawStrings(text) {
  let out = "";
  let i = 0;
  while (i < text.length) {
    const raw = /^(?:b|c)?r(#*)"/.exec(text.slice(i));
    if (raw !== null) {
      const prefixLength = raw[0].length;
      const terminator = `"${raw[1]}`;
      const close = text.indexOf(terminator, i + prefixLength);
      const end = close < 0 ? text.length : close;
      out += text.slice(i, i + prefixLength);
      out += stripCComments(text.slice(i + prefixLength, end));
      if (close < 0) {
        i = text.length;
      } else {
        out += terminator;
        i = close + terminator.length;
      }
      continue;
    }
    out += text[i];
    i++;
  }
  return out;
}

function stripCComments(text) {
  let out = "";
  let i = 0;
  while (i < text.length) {
    const c = text[i];
    if (c === '"' || c === "'") {
      const quote = c;
      out += c;
      i++;
      while (i < text.length) {
        const current = text[i];
        out += current;
        i++;
        if (current === "\\" && i < text.length) {
          out += text[i];
          i++;
          continue;
        }
        if (current === quote) {
          break;
        }
      }
      continue;
    }
    if (c === "/" && text[i + 1] === "/") {
      i += 2;
      while (i < text.length && text[i] !== "\n") {
        i++;
      }
      continue;
    }
    if (c === "/" && text[i + 1] === "*") {
      i += 2;
      out += " ";
      while (i < text.length) {
        if (text[i] === "*" && text[i + 1] === "/") {
          i += 2;
          break;
        }
        if (text[i] === "\n") {
          out += "\n";
        }
        i++;
      }
      continue;
    }
    out += c;
    i++;
  }
  return out;
}

function publicPackageJson() {
  const manifest = JSON.parse(readFileSync(join(root, "package.json"), "utf8"));
  const scripts = {};
  for (const key of [
    "build",
    "package",
    "package:npm",
    "clean",
    "typecheck",
    "astra",
    "nova",
    "nopm",
  ]) {
    if (manifest.scripts?.[key] !== undefined) {
      scripts[key] = manifest.scripts[key];
    }
  }
  return `${JSON.stringify(
    {
      name: manifest.name,
      version: manifest.version,
      private: true,
      license: manifest.license,
      description: manifest.description,
      type: manifest.type,
      engines: manifest.engines,
      workspaces: manifest.workspaces,
      scripts,
      devDependencies: manifest.devDependencies,
    },
    null,
    2,
  )}\n`;
}

function releaseWorkflow() {
  return `name: Release packages

on:
  push:
    tags: ["v*"]
  workflow_dispatch:

permissions:
  contents: write
  id-token: write
  attestations: write

jobs:
  package:
    name: package (\${{ matrix.os }})
    strategy:
      fail-fast: false
      matrix:
        os: [ubuntu-latest, macos-latest, windows-latest]
    runs-on: \${{ matrix.os }}
    permissions:
      contents: write
      id-token: write
      attestations: write
    steps:
      - uses: actions/checkout@v4

      - name: Install Rust (stable)
        uses: dtolnay/rust-toolchain@stable

      - name: Setup Node.js 22
        uses: actions/setup-node@v4
        with:
          node-version: 22

      - name: Install npm for trusted publishing
        run: npm install --global npm@11

      - name: Install workspace dependencies
        run: npm ci

      - name: Build standalone package
        run: npm run package

      - name: Attest release archives
        uses: actions/attest-build-provenance@v2
        with:
          subject-path: |
            dist/*.zip
            dist/*.tar.gz

      - name: Remove stale diagnostics
        shell: bash
        env:
          GH_TOKEN: \${{ github.token }}
        run: |
          tag="\${GITHUB_REF_NAME}"
          for name in \
            package-ubuntu-latest.log package-macos-latest.log package-windows-latest.log \
            release-ubuntu-latest.log release-macos-latest.log release-windows-latest.log; do
            gh release delete-asset "$tag" "$name" --yes >/dev/null 2>&1 || true
          done

      - name: Upload release assets
        shell: bash
        env:
          GH_TOKEN: \${{ github.token }}
        run: |
          tag="\${GITHUB_REF_NAME}"
          gh release view "$tag" >/dev/null 2>&1 || \\
            gh release create "$tag" --title "Neo Astra $tag" --notes "Neo Astra $tag" || true
          shopt -s nullglob
          files=(dist/*.zip dist/*.tar.gz dist/*.sha256)
          for file in "\${files[@]}"; do
            gh release upload "$tag" "$file" --clobber
          done

      - name: Build npm platform package
        run: node scripts/package-npm.mjs --platform

      - name: Publish npm platform package
        shell: bash
        env:
          NPM_CONFIG_FETCH_TIMEOUT: "1800000"
          NPM_CONFIG_FETCH_RETRIES: "5"
        run: |
          package_dir=dist/npm/platform
          name=$(node -p "require('./$package_dir/package.json').name")
          version=$(node -p "require('./$package_dir/package.json').version")
          if npm view "$name@$version" version >/dev/null 2>&1; then
            echo "$name@$version is already published."
            exit 0
          fi
          npm publish "$package_dir" --access public --tag latest --provenance

  meta:
    name: npm meta package
    needs: package
    runs-on: ubuntu-latest
    permissions:
      contents: read
      id-token: write
    steps:
      - uses: actions/checkout@v4

      - name: Setup Node.js 22
        uses: actions/setup-node@v4
        with:
          node-version: 22

      - name: Install npm for trusted publishing
        run: npm install --global npm@11

      - name: Build npm meta package
        run: node scripts/package-npm.mjs --meta

      - name: Publish npm meta package
        shell: bash
        env:
          NPM_CONFIG_FETCH_TIMEOUT: "1800000"
          NPM_CONFIG_FETCH_RETRIES: "5"
        run: |
          package_dir=dist/npm/meta
          name=$(node -p "require('./$package_dir/package.json').name")
          version=$(node -p "require('./$package_dir/package.json').version")
          if npm view "$name@$version" version >/dev/null 2>&1; then
            echo "$name@$version is already published."
            exit 0
          fi
          npm publish "$package_dir" --access public --tag latest --provenance
`;
}

function prepareOutput() {
  mkdirSync(out, { recursive: true });
  for (const entry of readdirSync(out, { withFileTypes: true })) {
    if (keepGit && entry.name === ".git") {
      continue;
    }
    rmSync(join(out, entry.name), { recursive: true, force: true });
  }
}

prepareOutput();
copyTree(root, out);
copyOverlay(join(root, "public"), out);

function copyOverlay(source, destination, relativePath = "") {
  for (const entry of readdirSync(source, { withFileTypes: true })) {
    const childRelative = relativePath === "" ? entry.name : `${relativePath}/${entry.name}`;
    const childSource = join(source, entry.name);
    const childDestination = join(destination, entry.name);
    if (entry.isDirectory()) {
      mkdirSync(childDestination, { recursive: true });
      copyOverlay(childSource, childDestination, childRelative);
    } else if (entry.isFile()) {
      const text = readFileSync(childSource, "utf8");
      const cleaned = cleanFile(childRelative, text);
      mkdirSync(dirname(childDestination), { recursive: true });
      writeFileSync(childDestination, cleaned, "utf8");
    }
  }
}

const publicPackage = publicPackageJson();
writeFileSync(join(out, "package.json"), publicPackage, "utf8");
writeFileSync(
  join(out, ".gitignore"),
  ["node_modules/", "dist/", "target/", ".tools/", "*.tsbuildinfo", ".DS_Store", ""].join("\n"),
  "utf8",
);
mkdirSync(join(out, ".github", "workflows"), { recursive: true });
writeFileSync(join(out, ".github", "workflows", "release.yml"), releaseWorkflow(), "utf8");

const fmt = spawnSync("cargo", ["fmt", "--all"], { cwd: out, stdio: "inherit" });
if (fmt.error) {
  process.stderr.write(`[public-source] cargo fmt failed: ${fmt.error.message}\n`);
  process.exit(1);
}
if (fmt.status !== 0) {
  process.exit(fmt.status ?? 1);
}

process.stdout.write(`[public-source] wrote ${out}\n`);
