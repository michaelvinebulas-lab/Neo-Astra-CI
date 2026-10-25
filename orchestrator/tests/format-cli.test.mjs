import test from "node:test";
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";

const root = fileURLToPath(new URL("../..", import.meta.url));
const cli = join(root, "orchestrator", "packages", "cli", "dist", "index.js");
const binary = join(
  root,
  "target",
  "debug",
  process.platform === "win32" ? "astra-compiler.exe" : "astra-compiler",
);

function runFmt(args) {
  return spawnSync(process.execPath, [cli, "fmt", ...args, "--compiler", binary], {
    cwd: root,
    encoding: "utf8",
  });
}

function createProject() {
  return mkdtempSync(join(tmpdir(), "astra-fmt-"));
}

function removeProject(directory) {
  rmSync(directory, { recursive: true, force: true });
}

test("astra fmt formats .ns files recursively and preserves comments", () => {
  const project = createProject();

  try {
    mkdirSync(join(project, "src", "nested"), { recursive: true });
    mkdirSync(join(project, "node_modules"), { recursive: true });

    const main = join(project, "src", "main.ns");
    const nested = join(project, "src", "nested", "util.ns");
    const ignored = join(project, "node_modules", "ignored.ns");

    writeFileSync(
      main,
      "// doc\npublic function f() {\n    // body\n    var mut x = 1 // value\n}\n",
      "utf8",
    );
    writeFileSync(nested, "public function g( ) {}", "utf8");
    writeFileSync(ignored, "public  function  ignored( ) {}", "utf8");

    const result = runFmt([project]);

    assert.equal(result.status, 0, result.stderr);
    const formatted = readFileSync(main, "utf8");
    assert.match(formatted, /pub fn f\(\)/);
    assert.match(formatted, /let mut x = 1/);
    assert.match(formatted, /\/\/ doc/);
    assert.match(formatted, /\/\/ body/);
    assert.match(formatted, /\/\/ value/);
    assert.match(readFileSync(nested, "utf8"), /pub fn g\(\)/);
    assert.equal(readFileSync(ignored, "utf8"), "public  function  ignored( ) {}");
  } finally {
    removeProject(project);
  }
});

test("astra fmt formats a single .ns file", () => {
  const project = createProject();

  try {
    const main = join(project, "main.ns");
    writeFileSync(main, "public  function  f( ) {}\n", "utf8");

    const result = runFmt([main]);

    assert.equal(result.status, 0, result.stderr);
    assert.equal(readFileSync(main, "utf8"), "pub fn f() {}\n");

    const other = join(project, "notes.txt");
    writeFileSync(other, "not source\n", "utf8");
    const rejected = runFmt([other]);
    assert.equal(rejected.status, 1);
    assert.match(rejected.stderr, /not a \.ns source file/);
  } finally {
    removeProject(project);
  }
});

test("astra fmt --check reports differences without writing", () => {
  const project = createProject();

  try {
    const main = join(project, "main.ns");
    const before = "public  function  f( ) {}\n";
    writeFileSync(main, before, "utf8");

    const check = runFmt([project, "--check"]);
    assert.equal(check.status, 1);
    assert.match(check.stdout, /would reformat/);
    assert.equal(readFileSync(main, "utf8"), before);

    const format = runFmt([project]);
    assert.equal(format.status, 0, format.stderr);

    const recheck = runFmt([project, "--check"]);
    assert.equal(recheck.status, 0, recheck.stderr);
    assert.match(recheck.stdout, /are formatted/);
  } finally {
    removeProject(project);
  }
});

test("astra fmt does not write any file when one source is invalid", () => {
  const project = createProject();

  try {
    const main = join(project, "main.ns");
    const broken = join(project, "broken.ns");
    const before = "public  function  f( ) {}\n";
    writeFileSync(main, before, "utf8");
    writeFileSync(broken, "function {\n", "utf8");

    const result = runFmt([project]);

    assert.equal(result.status, 1);
    assert.match(result.stderr, /NS2001|NS2003/);
    assert.equal(readFileSync(main, "utf8"), before);
  } finally {
    removeProject(project);
  }
});

test("astra fmt formats a manifest and .ns files together", () => {
  const project = createProject();

  try {
    mkdirSync(join(project, "src"), { recursive: true });
    const manifest = join(project, "core.json");
    const main = join(project, "src", "main.ns");

    writeFileSync(
      manifest,
      JSON.stringify({
        targets: { web: { mode: "web", backend: "js" } },
        package: { version: "1.0.0", name: "demo" },
      }),
      "utf8",
    );
    writeFileSync(main, "public  function  f( ) {}\n", "utf8");

    const result = runFmt([project]);

    assert.equal(result.status, 0, result.stderr);
    assert.match(readFileSync(manifest, "utf8"), /"package": \{\n    "name": "demo"/);
    assert.match(readFileSync(manifest, "utf8"), /"backend": "js",\n      "mode": "web"/);
    assert.equal(readFileSync(main, "utf8"), "pub fn f() {}\n");
  } finally {
    removeProject(project);
  }
});

test("astra fmt --check reports manifest differences without writing", () => {
  const project = createProject();

  try {
    const manifest = join(project, "core.json");
    const before = '{"package":{"version":"1.0.0","name":"demo"}}';
    writeFileSync(manifest, before, "utf8");

    const check = runFmt([project, "--check"]);
    assert.equal(check.status, 1);
    assert.match(check.stdout, /would reformat .*core\.json/);
    assert.equal(readFileSync(manifest, "utf8"), before);

    const format = runFmt([project]);
    assert.equal(format.status, 0, format.stderr);
    assert.match(readFileSync(manifest, "utf8"), /"name": "demo"/);
  } finally {
    removeProject(project);
  }
});

test("astra fmt formats a core.toml target and preserves comments", () => {
  const project = createProject();

  try {
    const manifest = join(project, "core.toml");
    writeFileSync(
      manifest,
      '# top\n[package]\nname="demo" # inline\nversion = "1.0.0"\n',
      "utf8",
    );

    const result = runFmt([manifest]);
    assert.equal(result.status, 0, result.stderr);

    const formatted = readFileSync(manifest, "utf8");
    assert.match(formatted, /# top/);
    assert.match(formatted, /name = "demo"\s+# inline/);

    const recheck = runFmt([manifest, "--check"]);
    assert.equal(recheck.status, 0, recheck.stderr);
  } finally {
    removeProject(project);
  }
});

test("astra fmt rejects an invalid manifest before writing source", () => {
  const project = createProject();

  try {
    const manifest = join(project, "core.json");
    const main = join(project, "main.ns");
    const before = "public  function  f( ) {}\n";

    writeFileSync(manifest, '{"package":', "utf8");
    writeFileSync(main, before, "utf8");

    const result = runFmt([project]);
    assert.equal(result.status, 1);
    assert.match(result.stderr, /NS9002/);
    assert.equal(readFileSync(main, "utf8"), before);
  } finally {
    removeProject(project);
  }
});

test("astra fmt --check --json returns a machine-readable report", () => {
  const project = createProject();

  try {
    writeFileSync(join(project, "main.ns"), "public  function  f( ) {}\n", "utf8");
    writeFileSync(
      join(project, "core.json"),
      '{"package":{"version":"1.0.0","name":"demo"}}',
      "utf8",
    );

    const result = runFmt([project, "--check", "--json"]);

    assert.equal(result.status, 1);
    const report = JSON.parse(result.stdout);
    assert.equal(report.check, true);
    assert.equal(report.ok, false);
    assert.equal(report.files.length, 2);
    assert.deepEqual(
      report.files
        .map((file) => file.path.split(/[\\/]/).pop())
        .sort(),
      ["core.json", "main.ns"],
    );
    assert.ok(report.files.every((file) => file.status === "would-reformat"));
  } finally {
    removeProject(project);
  }
});
