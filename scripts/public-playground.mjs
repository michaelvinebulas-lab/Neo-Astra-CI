#!/usr/bin/env node

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
import { spawnSync } from "node:child_process";

const root = fileURLToPath(new URL("..", import.meta.url));
const args = process.argv.slice(2);
const out = args.includes("--out")
  ? args[args.indexOf("--out") + 1]
  : join(root, "dist", "public-playground");
const keepGit = args.includes("--keep-git");

const rustCrates = [
  "astra-core",
  "astra-capabilities",
  "astra-compiler-api",
  "astra-wasm",
  "astra-codegen-c",
];
const tsPackages = [
  "compiler-client",
  "manifest",
  "codegen-js",
  "codegen-ts",
  "playground",
];

function tidyText(text) {
  return text
    .replace(/\r\n/g, "\n")
    .replace(/[ \t]+\n/g, "\n")
    .replace(/\n{3,}/g, "\n\n")
    .replace(/^\n+/, "")
    .replace(/\n+$/, "\n");
}

function readQuoted(text, start, quote) {
  let i = start + 1;
  while (i < text.length) {
    if (text[i] === "\\") {
      i += 2;
      continue;
    }
    if (text[i] === quote) return i + 1;
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
    if (text[i] === "`") return i + 1;
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
    if (c === "\n") return undefined;
    if (c === "[") inClass = true;
    else if (c === "]") inClass = false;
    else if (c === "/" && !inClass) {
      i++;
      while (i < text.length && /[A-Za-z]/.test(text[i])) i++;
      return i;
    }
    i++;
  }
  return undefined;
}

function stripJs(text) {
  let result = "";
  let i = 0;
  let regexAllowed = true;
  while (i < text.length) {
    const c = text[i];
    if (c === '"' || c === "'") {
      const end = readQuoted(text, i, c);
      result += text.slice(i, end);
      i = end;
      regexAllowed = false;
      continue;
    }
    if (c === "`") {
      const end = readTemplate(text, i);
      result += text.slice(i, end);
      i = end;
      regexAllowed = false;
      continue;
    }
    if (c === "/" && text[i + 1] === "/") {
      i += 2;
      while (i < text.length && text[i] !== "\n") i++;
      continue;
    }
    if (c === "/" && text[i + 1] === "*") {
      i += 2;
      result += " ";
      while (i < text.length) {
        if (text[i] === "*" && text[i + 1] === "/") {
          i += 2;
          break;
        }
        if (text[i] === "\n") result += "\n";
        i++;
      }
      continue;
    }
    if (c === "/" && regexAllowed) {
      const end = readRegex(text, i);
      if (end !== undefined) {
        result += text.slice(i, end);
        i = end;
        regexAllowed = false;
        continue;
      }
    }
    result += c;
    if (!/\s/.test(c)) {
      if (/[A-Za-z0-9_$]/.test(c) || c === ")" || c === "]" || c === "}") {
        regexAllowed = false;
      } else {
        regexAllowed = true;
      }
    }
    i++;
  }
  return result;
}

function stripRust(text) {
  let result = "";
  let i = 0;
  while (i < text.length) {
    const c = text[i];
    if (c === "/" && text[i + 1] === "/") {
      i += 2;
      while (i < text.length && text[i] !== "\n") i++;
      continue;
    }
    if (c === "/" && text[i + 1] === "*") {
      i += 2;
      let depth = 1;
      result += " ";
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
        if (text[i] === "\n") result += "\n";
        i++;
      }
      continue;
    }
    if (c === "r" || c === "b" || c === "c") {
      const raw = /^(?:b|c)?r(#*)"/.exec(text.slice(i));
      if (raw !== null) {
        const prefixLength = raw[0].length;
        const terminator = `"${raw[1]}`;
        const close = text.indexOf(terminator, i + prefixLength);
        const end = close < 0 ? text.length : close + terminator.length;
        result += text.slice(i, end);
        i = end;
        continue;
      }
    }
    if (c === '"') {
      const end = readQuoted(text, i, c);
      result += text.slice(i, end);
      i = end;
      continue;
    }
    if (c === "'") {
      const next = text[i + 1];
      const nextNext = text[i + 2];
      if (next === "\\" || nextNext === "'") {
        const end = readQuoted(text, i, c);
        result += text.slice(i, end);
        i = end;
        continue;
      }
    }
    result += c;
    i++;
  }
  return result;
}

function stripCComments(text) {
  let result = "";
  let i = 0;
  while (i < text.length) {
    const c = text[i];
    if (c === '"' || c === "'") {
      const quote = c;
      result += c;
      i++;
      while (i < text.length) {
        const current = text[i];
        result += current;
        i++;
        if (current === "\\" && i < text.length) {
          result += text[i];
          i++;
          continue;
        }
        if (current === quote) break;
      }
      continue;
    }
    if (c === "/" && text[i + 1] === "/") {
      i += 2;
      while (i < text.length && text[i] !== "\n") i++;
      continue;
    }
    if (c === "/" && text[i + 1] === "*") {
      i += 2;
      result += " ";
      while (i < text.length) {
        if (text[i] === "*" && text[i + 1] === "/") {
          i += 2;
          break;
        }
        if (text[i] === "\n") result += "\n";
        i++;
      }
      continue;
    }
    result += c;
    i++;
  }
  return result;
}

function stripCCommentsInRawStrings(text) {
  let result = "";
  let i = 0;
  while (i < text.length) {
    const raw = /^(?:b|c)?r(#*)"/.exec(text.slice(i));
    if (raw !== null) {
      const prefixLength = raw[0].length;
      const terminator = `"${raw[1]}`;
      const close = text.indexOf(terminator, i + prefixLength);
      const end = close < 0 ? text.length : close;
      result += text.slice(i, i + prefixLength);
      result += stripCComments(text.slice(i + prefixLength, end));
      if (close < 0) {
        i = text.length;
      } else {
        result += terminator;
        i = close + terminator.length;
      }
      continue;
    }
    result += text[i];
    i++;
  }
  return result;
}

function stripHashComments(text) {
  let result = "";
  let i = 0;
  while (i < text.length) {
    const c = text[i];
    if (c === '"' || c === "'") {
      const quote = c;
      result += c;
      i++;
      while (i < text.length) {
        const current = text[i];
        result += current;
        i++;
        if (current === "\\" && i < text.length) {
          result += text[i];
          i++;
          continue;
        }
        if (current === quote) break;
      }
      continue;
    }
    if (c === "#") {
      while (i < text.length && text[i] !== "\n") i++;
      continue;
    }
    result += c;
    i++;
  }
  return result;
}

function cleanFile(path, text) {
  const normalized = path.replaceAll("\\", "/");
  let cleaned;
  if (normalized.endsWith(".rs")) {
    const stripped = stripRust(text);
    cleaned =
      normalized === "crates/astra-codegen-c/src/runtime.rs"
        ? stripCCommentsInRawStrings(stripped)
        : stripped;
  } else if (
    normalized.endsWith(".ts") ||
    normalized.endsWith(".js") ||
    normalized.endsWith(".mjs") ||
    normalized.endsWith(".json") ||
    normalized.endsWith(".ns")
  ) {
    cleaned = stripJs(text);
  } else if (normalized.endsWith(".toml") || normalized.endsWith(".yml") || normalized.endsWith(".yaml")) {
    cleaned = stripHashComments(text);
  } else {
    cleaned = text;
  }
  return tidyText(cleaned);
}

function isSkipped(relativePath) {
  const normalized = relativePath.replaceAll("\\", "/");
  return (
    normalized.endsWith("/tests.rs") ||
    normalized.endsWith("/dev/smoke.mjs") ||
    normalized.includes("/tests/") ||
    normalized.endsWith("/tests") ||
    normalized.endsWith("/dist") ||
    normalized.includes("/dist/") ||
    normalized.endsWith("/node_modules") ||
    normalized.includes("/node_modules/") ||
    normalized.endsWith(".tsbuildinfo")
  );
}

function copyTree(source, destination, relativePath = "") {
  for (const entry of readdirSync(source, { withFileTypes: true })) {
    const childRelative = relativePath === "" ? entry.name : `${relativePath}/${entry.name}`;
    if (isSkipped(childRelative)) continue;
    const childSource = join(source, entry.name);
    const childDestination = join(destination, entry.name);
    if (entry.isDirectory()) {
      mkdirSync(childDestination, { recursive: true });
      copyTree(childSource, childDestination, childRelative);
    } else if (entry.isFile()) {
      const cleaned = cleanFile(childRelative, readFileSync(childSource, "utf8"));
      mkdirSync(dirname(childDestination), { recursive: true });
      writeFileSync(childDestination, cleaned, "utf8");
    }
  }
}

function copyApp(source, destination, relativePath = "") {
  for (const entry of readdirSync(source, { withFileTypes: true })) {
    if (entry.name === "node_modules" || entry.name === "dist") continue;
    const childRelative = relativePath === "" ? entry.name : `${relativePath}/${entry.name}`;
    const childSource = join(source, entry.name);
    const childDestination = join(destination, entry.name);
    if (entry.isDirectory()) {
      mkdirSync(childDestination, { recursive: true });
      copyApp(childSource, childDestination, childRelative);
    } else if (entry.isFile()) {
      const cleaned = cleanFile(childRelative, readFileSync(childSource, "utf8"));
      mkdirSync(dirname(childDestination), { recursive: true });
      writeFileSync(childDestination, cleaned, "utf8");
    }
  }
}

function prepareOutput() {
  mkdirSync(out, { recursive: true });
  for (const entry of readdirSync(out, { withFileTypes: true })) {
    if (keepGit && entry.name === ".git") continue;
    rmSync(join(out, entry.name), { recursive: true, force: true });
  }
}

const workspace = JSON.parse(readFileSync(join(root, "package.json"), "utf8"));

prepareOutput();

for (const crate of rustCrates) {
  copyTree(join(root, "crates", crate), join(out, "crates", crate), `crates/${crate}`);
}
for (const pkg of tsPackages) {
  copyTree(
    join(root, "orchestrator", "packages", pkg),
    join(out, "orchestrator", "packages", pkg),
    `orchestrator/packages/${pkg}`,
  );
}

writeFileSync(
  join(out, "Cargo.toml"),
  `[workspace]
members = [
${rustCrates.map((name) => `  "crates/${name}",`).join("\n")}
]
resolver = "3"

[workspace.package]
version = "${workspace.version}"
edition = "2024"
license = "${workspace.license}"

[workspace.dependencies]
astra-core = { path = "crates/astra-core" }
astra-capabilities = { path = "crates/astra-capabilities" }
astra-compiler-api = { path = "crates/astra-compiler-api" }
astra-codegen-c = { path = "crates/astra-codegen-c" }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
tempfile = "3"
toml = "0.8"

[profile.release]
strip = "symbols"
lto = "thin"
codegen-units = 1
`,
  "utf8",
);

writeFileSync(
  join(out, "package.json"),
  `${JSON.stringify(
    {
      name: "neo-astra-playground",
      version: workspace.version,
      private: true,
      license: workspace.license,
      description: "Browser playground for the Neo Astra compiler.",
      type: "module",
      engines: workspace.engines,
      workspaces: ["orchestrator/packages/*", "apps/playground"],
      scripts: {
        build: "tsc -b",
        clean: "tsc -b --clean",
        "build:wasm": "cargo build --release -p astra-wasm --target wasm32-unknown-unknown",
      },
      devDependencies: workspace.devDependencies,
    },
    null,
    2,
  )}\n`,
  "utf8",
);

writeFileSync(
  join(out, "tsconfig.json"),
  `${JSON.stringify(
    {
      files: [],
      references: [
        ...tsPackages.map((name) => ({ path: `orchestrator/packages/${name}` })),
        { path: "apps/playground" },
      ],
    },
    null,
    2,
  )}\n`,
  "utf8",
);
writeFileSync(join(out, "tsconfig.base.json"), readFileSync(join(root, "tsconfig.base.json"), "utf8"));
writeFileSync(
  join(out, ".gitignore"),
  ["node_modules/", "dist/", "target/", "*.tsbuildinfo", ".DS_Store", ""].join("\n"),
  "utf8",
);

copyApp(join(root, "playground-app"), join(out, "apps", "playground"));

mkdirSync(join(out, ".github", "workflows"), { recursive: true });
writeFileSync(
  join(out, ".github", "workflows", "playground.yml"),
  `name: Playground

on:
  push:
    branches: [main]
    tags: ["v*"]
  pull_request:

permissions:
  contents: read

concurrency:
  group: pages
  cancel-in-progress: true

jobs:
  build:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - name: Install Rust with wasm target
        uses: dtolnay/rust-toolchain@stable
        with:
          targets: wasm32-unknown-unknown

      - name: Build compiler core for wasm
        run: cargo build --release -p astra-wasm --target wasm32-unknown-unknown

      - name: Setup Node.js 22
        uses: actions/setup-node@v4
        with:
          node-version: 22

      - name: Install workspace dependencies
        run: npm ci

      - name: Build TypeScript app
        run: npm run build

      - name: Test playground app
        run: npm test --workspace @neo-astra/playground-app

      - name: Stage wasm artifact
        run: |
          mkdir -p apps/playground/dist
          cp target/wasm32-unknown-unknown/release/astra_wasm.wasm apps/playground/dist/astra_wasm.wasm

      - name: Upload Pages artifact
        uses: actions/upload-pages-artifact@v3
        with:
          path: apps/playground

  deploy:
    if: github.event_name != 'pull_request'
    needs: build
    runs-on: ubuntu-latest
    permissions:
      pages: write
      id-token: write
    environment:
      name: github-pages
      url: \${{ steps.deployment.outputs.page_url }}
    steps:
      - id: deployment
        uses: actions/deploy-pages@v4
`,
  "utf8",
);

writeFileSync(
  join(out, "README.md"),
  `# Neo Astra Playground

A static, browser-hosted playground for the Neo Astra language.

This repository is the sanitized public home of the playground introduced in
Neo Astra 1.2. The production compiler lives in
[Neo-Astra](https://github.com/luminousvalentine-cell/Neo-Astra); this repo
contains only the browser-safe bridge, codegen packages, and the static app.

## Local build

\`\`\`sh
rustup target add wasm32-unknown-unknown
npm ci
npm run build:wasm
npm run build
\`\`\`

Serve \`apps/playground/\` as a static directory. The compiled Wasm module is
copied into \`apps/playground/dist/\` by CI.

## Deployment

Pushes to \`main\` and tags matching \`v*\` deploy to GitHub Pages through
\`.github/workflows/playground.yml\`.
`,
  "utf8",
);

for (const name of ["LICENSE", "NOTICE"]) {
  const source = join(root, "public", name);
  if (existsSync(source)) {
    writeFileSync(join(out, name), readFileSync(source, "utf8"));
  }
}

const fmt = spawnSync("cargo", ["fmt", "--all"], { cwd: out, stdio: "inherit" });
if (fmt.error || fmt.status !== 0) {
  process.stderr.write(`[public-playground] cargo fmt failed: ${fmt.error?.message ?? fmt.status}\n`);
  process.exit(1);
}

const npmCommand = process.platform === "win32" ? "npm.cmd" : "npm";
const npm = spawnSync(
  npmCommand,
  ["install", "--package-lock-only", "--ignore-scripts", "--no-audit", "--no-fund"],
  { cwd: out, stdio: "inherit", shell: process.platform === "win32" },
);
if (npm.error || npm.status !== 0) {
  process.stderr.write(`[public-playground] npm lockfile generation failed: ${npm.error?.message ?? npm.status}\n`);
  process.exit(1);
}

process.stdout.write(`[public-playground] wrote ${out}\n`);
