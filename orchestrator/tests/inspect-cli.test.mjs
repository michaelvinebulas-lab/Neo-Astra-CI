import test from "node:test";
import assert from "node:assert/strict";
import { mkdir, mkdtemp, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";

import { runCli } from "../packages/cli/dist/index.js";

async function createProject(source = "@tags(fetch)\nmodule app.main\npub fn load() -> i32 {\n    return 1\n}\n") {
  const directory = await mkdtemp(join(tmpdir(), "neo-astra-inspect-"));
  await writeFile(
    join(directory, "core.json"),
    JSON.stringify({ package: { name: "inspect-demo" } }),
    "utf8",
  );
  await writeFile(
    join(directory, "build.json"),
    JSON.stringify({ nova: { entry: "src/main.ns" } }),
    "utf8",
  );
  await mkdir(join(directory, "src"), { recursive: true });
  await writeFile(join(directory, "src", "main.ns"), source, "utf8");
  return directory;
}

async function capture(run) {
  const stdout = [];
  const stderr = [];
  const originalStdout = process.stdout.write;
  const originalStderr = process.stderr.write;
  process.stdout.write = (chunk) => {
    stdout.push(String(chunk));
    return true;
  };
  process.stderr.write = (chunk) => {
    stderr.push(String(chunk));
    return true;
  };
  try {
    const code = await run();
    return { code, stdout: stdout.join(""), stderr: stderr.join("") };
  } finally {
    process.stdout.write = originalStdout;
    process.stderr.write = originalStderr;
  }
}

test("astra ast prints the parsed AST for a project entry", async () => {
  const directory = await createProject();
  try {
    const result = await capture(() => runCli(["ast", directory, "--json"], directory));
    assert.equal(result.code, 0, result.stderr);
    const response = JSON.parse(result.stdout);
    assert.equal(response.ok, true);
    assert.equal(response.apiVersion, "1.0");
    assert.ok(response.ast);
    assert.deepEqual(response.diagnostics, []);
  } finally {
    await rm(directory, { recursive: true, force: true });
  }
});

test("astra explain prints the inferred mode and reasons", async () => {
  const directory = await createProject();
  try {
    const result = await capture(() => runCli(["explain", directory], directory));
    assert.equal(result.code, 0, result.stderr);
    assert.match(result.stdout, /Resolved mode: web \(inferred\)/);
    assert.match(result.stdout, /capability tag 'fetch'/);
  } finally {
    await rm(directory, { recursive: true, force: true });
  }
});

test("astra explain accepts a file path and --json", async () => {
  const directory = await createProject();
  try {
    const file = join(directory, "src", "main.ns");
    const result = await capture(() => runCli(["explain", file, "--json"], directory));
    assert.equal(result.code, 0, result.stderr);
    const response = JSON.parse(result.stdout);
    assert.equal(response.ok, true);
    assert.equal(response.inference.capabilities[0], "fetch");
    assert.match(response.explanation, /Resolved mode: web/);
  } finally {
    await rm(directory, { recursive: true, force: true });
  }
});

test("astra ast reports parse diagnostics and exits non-zero", async () => {
  const directory = await createProject("pub fn main( {\n");
  try {
    const result = await capture(() => runCli(["ast", directory], directory));
    assert.equal(result.code, 1);
    assert.match(result.stderr, /NS2001/);
  } finally {
    await rm(directory, { recursive: true, force: true });
  }
});

test("astra ast rejects an unknown mode before running the compiler", async () => {
  const directory = await createProject();
  try {
    const result = await capture(() => runCli(["ast", directory, "--mode", "nope"], directory));
    assert.equal(result.code, 1);
    assert.match(result.stderr, /NS9000/);
    assert.match(result.stderr, /Unknown mode 'nope'/);
  } finally {
    await rm(directory, { recursive: true, force: true });
  }
});

test("astra ast --help uses the implemented command text", async () => {
  const result = await capture(() => runCli(["ast", "--help"], process.cwd()));
  assert.equal(result.code, 0);
  assert.match(result.stdout, /Print the parsed AST/);
  assert.doesNotMatch(result.stdout, /not implemented/);
});
