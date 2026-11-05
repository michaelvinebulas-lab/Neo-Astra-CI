import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import {
  existsSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  readdirSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";

import * as toolchain from "../packages/toolchain/dist/index.js";
import {
  createLockfile,
  formatLockfile,
  parseLockfile,
  validateLockfile,
} from "../packages/nopm/dist/index.js";

const root = fileURLToPath(new URL("../..", import.meta.url));
const cli = join(root, "orchestrator", "packages", "cli", "dist", "index.js");
const nopmCli = join(root, "orchestrator", "packages", "nopm", "dist", "cli.js");
const astraCompiler = join(
  root,
  "target",
  "debug",
  process.platform === "win32" ? "astra-compiler.exe" : "astra-compiler",
);

const ROOT = "/proj";
const CACHE = (id) => `${ROOT}/.tools/toolchain/${id}`;
const OK = (stdout = "") => ({ status: 0, stdout, stderr: "" });
const MISS = { status: 1, stdout: "", stderr: "not found" };

function has(record, key) {
  return Object.prototype.hasOwnProperty.call(record, key);
}

function fakeProbe({ files = {}, commands = {} } = {}) {
  const calls = [];
  return {
    calls,
    exists: (path) => has(files, path),
    readFileSync: (path) => {
      if (!has(files, path)) {
        throw new Error(`ENOENT: ${path}`);
      }
      return files[path];
    },
    readdir: () => [],
    run: (command, args) => {
      calls.push([command, ...args]);
      const full = [command, ...args].join(" ");
      if (has(commands, full)) {
        return commands[full];
      }
      if (args.length === 1 && args[0] === "--version" && has(commands, command)) {
        return commands[command];
      }
      return MISS;
    },
  };
}

function expectCode(fn, code) {
  assert.throws(fn, (error) => {
    const actual =
      error !== null && typeof error === "object" && "code" in error
        ? error.code
        : error instanceof Error
          ? error.message
          : String(error);
    assert.equal(actual, code, `expected ${code}, got ${String(error)}`);
    return true;
  });
}

async function expectCodeAsync(promise, code) {
  let caught;
  await assert.rejects(promise, (error) => {
    caught = error;
    const actual =
      error !== null && typeof error === "object" && "code" in error
        ? error.code
        : error instanceof Error
          ? error.message
          : String(error);
    assert.equal(actual, code, `expected ${code}, got ${String(error)}`);
    return true;
  });
  return caught;
}

function hostPlatform() {
  return process.platform;
}

function hostArch() {
  if (process.arch === "x64" || process.arch === "arm64") return process.arch;
  return process.arch;
}

/**
 * Builds a tiny archive with the same shape as a real Zig release: one
 * top-level directory containing the `zig` binary. Windows uses zip; POSIX uses
 * tar.xz, so the production extractor is exercised on every host.
 */
function makeToolchainArchive(binaryName = process.platform === "win32" ? "zig.exe" : "zig") {
  const source = mkdtempSync(join(tmpdir(), "astra-archive-"));
  try {
    const top = join(source, "zig-test");
    mkdirSync(top, { recursive: true });
    writeFileSync(join(top, binaryName), "fake zig binary\n", "utf8");
    const archivePath = join(source, process.platform === "win32" ? "archive.zip" : "archive.tar.xz");
    const tarArgs =
      process.platform === "win32"
        ? ["-a", "-cf", archivePath, "-C", source, "zig-test"]
        : ["-cJf", archivePath, "-C", source, "zig-test"];
    const tar = process.platform === "win32"
      ? `${process.env["SystemRoot"] ?? "C:\\Windows"}\\System32\\tar.exe`
      : "tar";
    const result = spawnSync(tar, tarArgs, { windowsHide: true, encoding: "utf8" });
    assert.equal(result.status, 0, `archive creation failed: ${result.stderr}`);
    const bytes = readFileSync(archivePath);
    return {
      bytes,
      sha256: createHash("sha256").update(bytes).digest("hex"),
      size: bytes.length,
    };
  } finally {
    rmSync(source, { recursive: true, force: true });
  }
}

function fakeArtifact(archive, overrides = {}) {
  return {
    id: "zig@0.13.0",
    version: "0.13.0",
    family: "zig",
    platform: hostPlatform(),
    arch: hostArch(),
    url: "https://example.invalid/zig-test-archive",
    sha256: archive.sha256,
    size: archive.size,
    format: process.platform === "win32" ? "zip" : "tar.xz",
    ...overrides,
  };
}

function fakeFetch(bytes, { status = 200 } = {}) {
  const calls = [];
  const fetchImpl = async (url, init) => {
    calls.push([url, init]);
    return {
      ok: status >= 200 && status < 300,
      status,
      async arrayBuffer() {
        return bytes.buffer.slice(bytes.byteOffset, bytes.byteOffset + bytes.byteLength);
      },
    };
  };
  return { calls, fetchImpl };
}

function makeEmptyProject() {
  return mkdtempSync(join(tmpdir(), "astra-pinned-"));
}

function parsePolicy(c) {
  return toolchain.parseToolchainPolicy({ toolchain: { c } });
}

function resolve(options = {}) {
  return toolchain.resolveToolchain({
    projectRoot: ROOT,
    policy: { request: "auto" },
    env: {},
    probe: fakeProbe(),
    ...options,
  });
}

test("toolchain policy defaults to auto and parses every pin family", () => {
  assert.deepEqual(toolchain.parseToolchainPolicy({}), { request: "auto" });
  assert.deepEqual(toolchain.parseToolchainPolicy(undefined), { request: "auto" });
  assert.deepEqual(parsePolicy("auto"), { request: "auto" });
  for (const pin of ["clang", "gcc@14", "zig@0.13.0", "cc"]) {
    assert.deepEqual(parsePolicy(pin), { request: pin });
  }
});

test("toolchain policy rejects bad values with NS9101", () => {
  for (const bad of ["", "   ", "rust", "clang@", "clang@@18", "clang@ 18", 7, null]) {
    expectCode(() => parsePolicy(bad), "NS9101");
  }
  expectCode(() => toolchain.parseToolchainPolicy({ toolchain: "clang" }), "NS9101");
  expectCode(() => toolchain.parseToolchainPolicy({ toolchain: { c: {} } }), "NS9101");
});

test("errors are ToolchainError instances carrying the code", () => {
  assert.equal(typeof toolchain.ToolchainError, "function");
  assert.throws(
    () => parsePolicy("rust"),
    (error) => {
      assert.ok(error instanceof toolchain.ToolchainError);
      assert.equal(error.code, "NS9101");
      assert.ok(error.message.length > 0);
      return true;
    },
  );
});

test("ASTRA_CC wins and records the env rung", () => {
  const probe = fakeProbe({
    files: { "/opt/clang": "binary" },
    commands: { "/opt/clang": OK("Ubuntu clang version 18.1.3\nTarget: x86_64") },
  });
  const resolution = resolve({ env: { ASTRA_CC: "/opt/clang" }, probe });
  assert.deepEqual(resolution, {
    rung: "env",
    id: "clang",
    family: "clang",
    path: "/opt/clang",
    version: "Ubuntu clang version 18.1.3",
  });
});

test("ASTRA_CC must exist and answer --version", () => {
  expectCode(() => resolve({ env: { ASTRA_CC: "/missing/clang" } }), "NS9103");
  const probe = fakeProbe({
    files: { "/missing/clang": "binary" },
    commands: { "/missing/clang": MISS },
  });
  expectCode(() => resolve({ env: { ASTRA_CC: "/missing/clang" }, probe }), "NS9103");
});

test("auto prefers the first compiler that passes the hosted probe", () => {
  const probe = fakeProbe({
    commands: {
      clang: OK("clang 18"),
      gcc: OK("gcc 14"),
      "clang -std=c17 -fsyntax-only -x c -": MISS,
      "gcc -std=c17 -fsyntax-only -x c -": OK(),
    },
  });
  const resolution = resolve({ probe });
  assert.equal(resolution.rung, "path");
  assert.equal(resolution.path, "gcc");
  assert.equal(resolution.family, "gcc");
  assert.equal(resolution.id, "gcc");
});

test("auto falls back to the first --version responder", () => {
  const probe = fakeProbe({ commands: { clang: OK("clang 18") } });
  const resolution = resolve({ probe });
  assert.equal(resolution.rung, "path");
  assert.equal(resolution.path, "clang");
});

test("auto maps the cc binary to the gcc family", () => {
  const probe = fakeProbe({ commands: { cc: OK("cc") } });
  const resolution = resolve({ probe });
  assert.equal(resolution.path, "cc");
  assert.equal(resolution.family, "gcc");
});

test("auto fails NS9102 when nothing answers", () => {
  expectCode(() => resolve({ probe: fakeProbe() }), "NS9102");
});

test("a versioned pin resolves from the cache", () => {
  const dir = CACHE("zig@0.13.0");
  const probe = fakeProbe({
    files: {
      [`${dir}/toolchain.json`]: JSON.stringify({
        id: "zig@0.13.0",
        family: "zig",
        version: "0.13.0",
      }),
      [`${dir}/bin/zig`]: "binary",
    },
  });
  const resolution = resolve({ policy: { request: "zig@0.13.0" }, probe });
  assert.deepEqual(resolution, {
    rung: "cache",
    id: "zig@0.13.0",
    family: "zig",
    path: `${dir}/bin/zig`,
    version: "0.13.0",
  });
});

test("a cache binary at the pin root is accepted", () => {
  const dir = CACHE("clang@18");
  const probe = fakeProbe({
    files: {
      [`${dir}/toolchain.json`]: JSON.stringify({
        id: "clang@18",
        family: "clang",
        version: "18",
      }),
      [`${dir}/clang`]: "binary",
    },
  });
  const resolution = resolve({ policy: { request: "clang@18" }, probe });
  assert.equal(resolution.rung, "cache");
  assert.equal(resolution.path, `${dir}/clang`);
});

test("a versioned pin never falls back to PATH", () => {
  const probe = fakeProbe({ commands: { zig: OK("zig 0.13") } });
  expectCode(() => resolve({ policy: { request: "zig@0.13.0" }, probe }), "NS9102");
  assert.ok(
    !probe.calls.some(([command]) => command === "zig"),
    "a versioned pin must not probe PATH",
  );
});

test("a bare family pin falls back to PATH when the cache is absent", () => {
  const probe = fakeProbe({ commands: { clang: OK("clang 18") } });
  const resolution = resolve({ policy: { request: "clang" }, probe });
  assert.equal(resolution.rung, "path");
  assert.equal(resolution.id, "clang");
  assert.equal(resolution.family, "clang");
  assert.equal(resolution.path, "clang");
});

test("a marker whose id differs fails NS9102, bare and versioned", () => {
  for (const id of ["zig@0.13.0", "zig"]) {
    const dir = CACHE(id);
    const probe = fakeProbe({
      files: {
        [`${dir}/toolchain.json`]: JSON.stringify({
          id: "zig@0.12.0",
          family: "zig",
          version: "0.12.0",
        }),
        [`${dir}/bin/zig`]: "binary",
      },
      commands: { zig: OK("zig 0.12") },
    });
    expectCode(() => resolve({ policy: { request: id }, probe }), "NS9102");
  }
});

test("a marker for another family fails NS9102", () => {
  const dir = CACHE("zig@0.13.0");
  const probe = fakeProbe({
    files: {
      [`${dir}/toolchain.json`]: JSON.stringify({
        id: "zig@0.13.0",
        family: "clang",
        version: "0.13.0",
      }),
      [`${dir}/bin/clang`]: "binary",
      [`${dir}/bin/zig`]: "binary",
    },
  });
  expectCode(() => resolve({ policy: { request: "zig@0.13.0" }, probe }), "NS9102");
});

test("a versioned pin rejects a marker with another version", () => {
  const dir = CACHE("zig@0.13.0");
  const probe = fakeProbe({
    files: {
      [`${dir}/toolchain.json`]: JSON.stringify({
        id: "zig@0.13.0",
        family: "zig",
        version: "0.12.0",
      }),
      [`${dir}/bin/zig`]: "binary",
    },
  });
  expectCode(() => resolve({ policy: { request: "zig@0.13.0" }, probe }), "NS9102");
});

test("a marker without a binary fails NS9102", () => {
  const dir = CACHE("zig@0.13.0");
  const probe = fakeProbe({
    files: {
      [`${dir}/toolchain.json`]: JSON.stringify({
        id: "zig@0.13.0",
        family: "zig",
        version: "0.13.0",
      }),
    },
  });
  expectCode(() => resolve({ policy: { request: "zig@0.13.0" }, probe }), "NS9102");
});

test("doctor reports the winning rung and every rung tried", () => {
  const dir = CACHE("zig@0.13.0");
  const probe = fakeProbe({
    files: {
      [`${dir}/toolchain.json`]: JSON.stringify({
        id: "zig@0.13.0",
        family: "zig",
        version: "0.13.0",
      }),
      [`${dir}/bin/zig`]: "binary",
    },
  });
  const result = toolchain.doctor(ROOT, { request: "zig@0.13.0" }, {}, probe);
  assert.equal(result.winner?.rung, "cache");
  assert.ok(result.attempts.some((entry) => entry.rung === "cache"));
  assert.equal(result.error, null);
});

test("doctor reports the failure reason when nothing resolves", () => {
  const result = toolchain.doctor(ROOT, { request: "zig@0.13.0" }, {}, fakeProbe());
  assert.equal(result.winner, null);
  assert.match(String(result.error), /NS9102/);
  assert.ok(result.attempts.length > 0);
});

test("doctor does not duplicate the env rung", () => {
  const probe = fakeProbe({
    files: { "/opt/clang": "binary" },
    commands: { "/opt/clang": OK("clang 18") },
  });
  const result = toolchain.doctor(ROOT, { request: "auto" }, { ASTRA_CC: "/opt/clang" }, probe);
  assert.equal(result.winner?.rung, "env");
  assert.equal(result.attempts.filter((entry) => entry.rung === "env").length, 1);
});

function sampleLockfile(overrides = {}) {
  return createLockfile({
    resolution: { targets: [], packages: [] },
    generatedBy: "astra/0.1.0",
    manifestHash: "sha256-manifest",
    compiler: { version: "0.1.0", apiVersion: "1.0" },
    ...overrides,
  });
}

test("lockfile records, formats, and round-trips a toolchain", () => {
  const record = {
    rung: "path",
    id: "clang",
    family: "clang",
    path: "/usr/bin/clang",
    version: "18.1.3",
  };
  const lockfile = sampleLockfile({ toolchain: record });
  assert.deepEqual(lockfile.toolchain, record);
  const formatted = formatLockfile(lockfile);
  assert.match(formatted, /"toolchain"/);
  assert.deepEqual(parseLockfile(formatted), lockfile);
  assert.equal(formatLockfile(parseLockfile(formatted)), formatted);
});

test("lockfile defaults the toolchain to null", () => {
  const lockfile = sampleLockfile();
  assert.equal(lockfile.toolchain, null);
  assert.equal(parseLockfile(formatLockfile(lockfile)).toolchain, null);
});

test("lockfile rejects an unknown toolchain rung", () => {
  const lockfile = sampleLockfile({
    toolchain: {
      rung: "path",
      id: "clang",
      family: "clang",
      path: "clang",
      version: null,
    },
  });
  const value = JSON.parse(formatLockfile(lockfile));
  value.toolchain.rung = "download";
  expectCode(() => validateLockfile(value), "NP0008");
});

function makeProject(toolchainPolicy = "auto") {
  const project = mkdtempSync(join(tmpdir(), "astra-toolchain-"));
  mkdirSync(join(project, "src"), { recursive: true });
  writeFileSync(
    join(project, "core.json"),
    JSON.stringify(
      {
        package: {
          name: "demo",
          version: "1.0.0",
          edition: "2026",
          license: "MIT",
        },
      },
      null,
      2,
    ),
    "utf8",
  );
  writeFileSync(
    join(project, "build.json"),
    JSON.stringify(
      {
        build: { defaultTarget: "native", defaultMode: "core" },
        nova: { entry: "src/main.ns" },
        targets: {
          native: { backend: "native", mode: "core", outDir: "dist/native" },
        },
        math: { integers: "wrapping" },
        toolchain: { c: toolchainPolicy },
      },
      null,
      2,
    ),
    "utf8",
  );
  writeFileSync(
    join(project, "src", "main.ns"),
    "pub fn main() -> i32 {\n    return 0\n}\n",
    "utf8",
  );
  return project;
}

function runNode(args, options = {}) {
  return spawnSync(process.execPath, args, {
    cwd: root,
    encoding: "utf8",
    windowsHide: true,
    ...options,
  });
}

test("astra doctor prints the env rung and the resolved binary", () => {
  const project = makeProject();
  try {
    const result = runNode([cli, "doctor", project], {
      env: { ...process.env, ASTRA_CC: process.execPath },
    });
    assert.equal(result.status, 0, result.stderr);
    assert.match(result.stdout, /policy: auto/);
    assert.match(result.stdout, /resolved:/);
    assert.ok(
      result.stdout.includes(process.execPath),
      `doctor output must name the binary:\n${result.stdout}`,
    );
  } finally {
    rmSync(project, { recursive: true, force: true });
  }
});

test("astra doctor fails NS9102 for an unresolved versioned pin", () => {
  const project = makeProject("clang@999999");
  try {
    const result = runNode([cli, "doctor", project], {
      env: { ...process.env, ASTRA_CC: "" },
    });
    assert.equal(result.status, 1);
    assert.match(`${result.stdout}${result.stderr}`, /NS9102/);
  } finally {
    rmSync(project, { recursive: true, force: true });
  }
});

test("astra build surfaces NS9102 for an unresolved versioned pin", () => {
  const project = makeProject("clang@999999");
  try {
    const result = runNode([
      cli,
      "build",
      project,
      "--compiler",
      astraCompiler,
      "--target",
      "native",
    ]);
    assert.notEqual(result.status, 0);
    const output = `${result.stdout}${result.stderr}`;
    assert.match(output, /NS9102/);
    assert.doesNotMatch(output, /NS9008/);
  } finally {
    rmSync(project, { recursive: true, force: true });
  }
});

test("nopm lock records the resolved toolchain", () => {
  const project = makeProject();
  try {
    const result = runNode([nopmCli, "lock", project], {
      env: { ...process.env, ASTRA_CC: process.execPath },
    });
    assert.equal(result.status, 0, result.stderr);
    const lockfile = JSON.parse(
      readFileSync(join(project, "core.lock.json"), "utf8"),
    );
    assert.equal(lockfile.toolchain?.rung, "env");
    assert.equal(lockfile.toolchain?.path, process.execPath);
  } finally {
    rmSync(project, { recursive: true, force: true });
  }
});

test("nopm lock records null when an auto toolchain does not resolve", () => {
  const project = makeProject();
  const emptyPath = mkdtempSync(join(tmpdir(), "astra-empty-path-"));
  try {
    const env = { ...process.env, PATH: emptyPath };
    delete env.ASTRA_CC;
    const result = runNode([nopmCli, "lock", project], { env });
    assert.equal(result.status, 0, result.stderr);
    const lockfile = JSON.parse(
      readFileSync(join(project, "core.lock.json"), "utf8"),
    );
    assert.equal(lockfile.toolchain, null);
  } finally {
    rmSync(project, { recursive: true, force: true });
    rmSync(emptyPath, { recursive: true, force: true });
  }
});

test("nopm lock fails NS9102 for an unresolved versioned pin", () => {
  const project = makeProject("clang@999999");
  try {
    const result = runNode([nopmCli, "lock", project], {
      env: { ...process.env, ASTRA_CC: "" },
    });
    assert.notEqual(result.status, 0);
    assert.match(`${result.stdout}${result.stderr}`, /NS9102/);
  } finally {
    rmSync(project, { recursive: true, force: true });
  }
});

test("nopm lock records a pre-seeded pinned cache toolchain", () => {
  const project = makeProject("zig@0.13.0");
  const cache = join(project, ".tools", "toolchain", "zig@0.13.0");
  const binary = process.platform === "win32" ? "zig.exe" : "zig";
  mkdirSync(cache, { recursive: true });
  writeFileSync(
    join(cache, "toolchain.json"),
    JSON.stringify({
      id: "zig@0.13.0",
      family: "zig",
      version: "0.13.0",
    }),
    "utf8",
  );
  writeFileSync(join(cache, binary), "fake zig binary\n", "utf8");
  try {
    const result = runNode([nopmCli, "lock", project], {
      env: { ...process.env, ASTRA_CC: "" },
    });
    assert.equal(result.status, 0, result.stderr);
    const lockfile = JSON.parse(
      readFileSync(join(project, "core.lock.json"), "utf8"),
    );
    assert.equal(lockfile.toolchain?.rung, "cache");
    assert.equal(lockfile.toolchain?.id, "zig@0.13.0");
    assert.equal(lockfile.toolchain?.family, "zig");
    assert.ok(String(lockfile.toolchain?.path).endsWith(binary));
  } finally {
    rmSync(project, { recursive: true, force: true });
  }
});

const OFFICIAL_ARTIFACTS = [
  [
    "linux",
    "x64",
    "https://ziglang.org/download/0.13.0/zig-linux-x86_64-0.13.0.tar.xz",
    "d45312e61ebcc48032b77bc4cf7fd6915c11fa16e4aad116b66c9468211230ea",
    47082308,
  ],
  [
    "linux",
    "arm64",
    "https://ziglang.org/download/0.13.0/zig-linux-aarch64-0.13.0.tar.xz",
    "041ac42323837eb5624068acd8b00cd5777dac4cf91179e8dad7a7e90dd0c556",
    43090688,
  ],
  [
    "darwin",
    "x64",
    "https://ziglang.org/download/0.13.0/zig-macos-x86_64-0.13.0.tar.xz",
    "8b06ed1091b2269b700b3b07f8e3be3b833000841bae5aa6a09b1a8b4773effd",
    48857012,
  ],
  [
    "darwin",
    "arm64",
    "https://ziglang.org/download/0.13.0/zig-macos-aarch64-0.13.0.tar.xz",
    "46fae219656545dfaf4dce12fb4e8685cec5b51d721beee9389ab4194d43394c",
    44892040,
  ],
  [
    "win32",
    "x64",
    "https://ziglang.org/download/0.13.0/zig-windows-x86_64-0.13.0.zip",
    "d859994725ef9402381e557c60bb57497215682e355204d754ee3df75ee3c158",
    79163968,
  ],
  [
    "win32",
    "arm64",
    "https://ziglang.org/download/0.13.0/zig-windows-aarch64-0.13.0.zip",
    "95ff88427af7ba2b4f312f45d2377ce7a033e5e3c620c8caaa396a9aba20efda",
    75119033,
  ],
];

test("pinned artifacts are the official Zig 0.13.0 archives", () => {
  assert.equal(toolchain.PINNED_ARTIFACTS.length, OFFICIAL_ARTIFACTS.length);
  for (const [platform, arch, url, sha256, size] of OFFICIAL_ARTIFACTS) {
    const artifact = toolchain.pinnedArtifact("zig@0.13.0", platform, arch);
    assert.ok(artifact, `${platform}/${arch} must be pinned`);
    assert.equal(artifact.url, url);
    assert.equal(artifact.sha256, sha256);
    assert.equal(artifact.size, size);
    assert.equal(artifact.version, "0.13.0");
    assert.equal(artifact.family, "zig");
  }
  assert.equal(toolchain.pinnedArtifact("zig@0.12.0", "linux", "x64"), null);
  assert.equal(toolchain.pinnedArtifact("zig@0.13.0", "freebsd", "x64"), null);
  assert.equal(toolchain.pinnedArtifact("zig@0.13.0", "linux", "ia32"), null);
  assert.equal(toolchain.pinnedArtifact("zig@0.13.0", "linux", "x86_64")?.arch, "x64");
  assert.equal(toolchain.pinnedArtifact("zig@0.13.0", "darwin", "aarch64")?.arch, "arm64");
  assert.equal(
    toolchain.pinnedArtifact("zig@0.13.0", "linux", "x64", []),
    null,
  );
});

test("zig resolves through zig cc and zig ar", () => {
  const resolution = {
    rung: "cache",
    id: "zig@0.13.0",
    family: "zig",
    path: "/cache/zig",
    version: "0.13.0",
  };
  assert.deepEqual(toolchain.compilerInvocation(resolution), {
    bin: "/cache/zig",
    args: ["cc"],
  });
  assert.deepEqual(toolchain.archiverInvocation(resolution), {
    bin: "/cache/zig",
    args: ["ar"],
  });
  const clang = {
    rung: "path",
    id: "clang",
    family: "clang",
    path: "clang",
    version: "18",
  };
  assert.deepEqual(toolchain.compilerInvocation(clang), {
    bin: "clang",
    args: [],
  });
  assert.deepEqual(toolchain.archiverInvocation(clang), {
    bin: "clang",
    args: [],
  });
});

test("pinned toolchain download verifies and installs into the project cache", async () => {
  const project = makeEmptyProject();
  const archive = makeToolchainArchive();
  const artifact = fakeArtifact(archive);
  const fetched = fakeFetch(archive.bytes);
  try {
    const resolution = await toolchain.installPinnedToolchain(
      project,
      "zig@0.13.0",
      { artifacts: [artifact], fetchImpl: fetched.fetchImpl },
    );
    assert.equal(fetched.calls.length, 1);
    assert.equal(fetched.calls[0][0], artifact.url);
    assert.equal(resolution.rung, "cache");
    assert.equal(resolution.id, "zig@0.13.0");
    assert.equal(resolution.family, "zig");
    assert.equal(resolution.version, "0.13.0");
    const binaryName = process.platform === "win32" ? "zig.exe" : "zig";
    assert.ok(resolution.path.endsWith(binaryName), resolution.path);
    assert.ok(resolution.path.includes("zig@0.13.0"), resolution.path);
    const cache = join(project, ".tools", "toolchain", "zig@0.13.0");
    const marker = JSON.parse(
      readFileSync(join(cache, "toolchain.json"), "utf8"),
    );
    assert.equal(marker.id, "zig@0.13.0");
    assert.equal(marker.family, "zig");
    assert.equal(marker.version, "0.13.0");
    assert.equal(marker.sha256, archive.sha256);
    assert.ok(existsSync(join(cache, binaryName)));
    assert.deepEqual(
      readdirSync(join(project, ".tools", "toolchain")),
      ["zig@0.13.0"],
    );
  } finally {
    rmSync(project, { recursive: true, force: true });
  }
});

test("a second pinned resolve uses the verified cache without fetching", async () => {
  const project = makeEmptyProject();
  const archive = makeToolchainArchive();
  const artifact = fakeArtifact(archive);
  const first = fakeFetch(archive.bytes);
  try {
    const installed = await toolchain.installPinnedToolchain(
      project,
      "zig@0.13.0",
      { artifacts: [artifact], fetchImpl: first.fetchImpl },
    );
    const unexpected = fakeFetch(Buffer.from("unused"));
    const resolved = await toolchain.resolveToolchainAsync({
      projectRoot: project,
      policy: { request: "zig@0.13.0" },
      env: {},
      allowDownload: true,
      artifacts: [artifact],
      fetchImpl: unexpected.fetchImpl,
    });
    assert.equal(unexpected.calls.length, 0);
    assert.equal(resolved.rung, "cache");
    assert.equal(resolved.path, installed.path);
  } finally {
    rmSync(project, { recursive: true, force: true });
  }
});

test("a corrupt pinned cache is authoritative and is not overwritten", async () => {
  const project = makeEmptyProject();
  const archive = makeToolchainArchive();
  const artifact = fakeArtifact(archive);
  const fetched = fakeFetch(archive.bytes);
  const cache = join(project, ".tools", "toolchain", "zig@0.13.0");
  mkdirSync(cache, { recursive: true });
  writeFileSync(
    join(cache, "toolchain.json"),
    JSON.stringify({ id: "zig@0.13.0", family: "zig", version: "0.12.0" }),
    "utf8",
  );
  writeFileSync(join(cache, process.platform === "win32" ? "zig.exe" : "zig"), "old", "utf8");
  try {
    const error = await expectCodeAsync(
      toolchain.installPinnedToolchain(project, "zig@0.13.0", {
        artifacts: [artifact],
        fetchImpl: fetched.fetchImpl,
      }),
      "NS9102",
    );
    assert.match(String(error.message), /Manual install/);
    assert.equal(fetched.calls.length, 0);
    const marker = JSON.parse(
      readFileSync(join(cache, "toolchain.json"), "utf8"),
    );
    assert.equal(marker.version, "0.12.0");
  } finally {
    rmSync(project, { recursive: true, force: true });
  }
});

test("pinned download rejects HTTP, transport, size, hash, and archive failures", async () => {
  const project = makeEmptyProject();
  const archive = makeToolchainArchive();
  const artifact = fakeArtifact(archive);
  const httpFailure = async () => ({
    ok: false,
    status: 403,
    async arrayBuffer() {
      return new ArrayBuffer(0);
    },
  });
  try {
    await expectCodeAsync(
      toolchain.installPinnedToolchain(project, "zig@0.13.0", {
        artifacts: [artifact],
        fetchImpl: httpFailure,
      }),
      "NS9102",
    );
    await expectCodeAsync(
      toolchain.installPinnedToolchain(project, "zig@0.13.0", {
        artifacts: [artifact],
        fetchImpl: async () => {
          throw new Error("network down");
        },
      }),
      "NS9102",
    );
    await expectCodeAsync(
      toolchain.installPinnedToolchain(project, "zig@0.13.0", {
        artifacts: [fakeArtifact(archive, { size: archive.size + 1 })],
        fetchImpl: fakeFetch(archive.bytes).fetchImpl,
      }),
      "NS9102",
    );
    await expectCodeAsync(
      toolchain.installPinnedToolchain(project, "zig@0.13.0", {
        artifacts: [fakeArtifact(archive, { sha256: "0".repeat(64) })],
        fetchImpl: fakeFetch(archive.bytes).fetchImpl,
      }),
      "NS9102",
    );

    const badBytes = Buffer.from("this is not a toolchain archive", "utf8");
    const badArchive = {
      bytes: badBytes,
      sha256: createHash("sha256").update(badBytes).digest("hex"),
      size: badBytes.length,
    };
    await expectCodeAsync(
      toolchain.installPinnedToolchain(project, "zig@0.13.0", {
        artifacts: [fakeArtifact(badArchive)],
        fetchImpl: fakeFetch(badArchive.bytes).fetchImpl,
      }),
      "NS9102",
    );

    const missingBinary = makeToolchainArchive("not-zig");
    await expectCodeAsync(
      toolchain.installPinnedToolchain(project, "zig@0.13.0", {
        artifacts: [fakeArtifact(missingBinary)],
        fetchImpl: fakeFetch(missingBinary.bytes).fetchImpl,
      }),
      "NS9102",
    );
  } finally {
    rmSync(project, { recursive: true, force: true });
  }
});

test("an unknown pinned artifact fails before any network request", async () => {
  const project = makeEmptyProject();
  const fetched = fakeFetch(Buffer.from("unused"));
  try {
    const error = await expectCodeAsync(
      toolchain.installPinnedToolchain(project, "zig@0.13.0", {
        artifacts: [],
        fetchImpl: fetched.fetchImpl,
      }),
      "NS9102",
    );
    assert.equal(fetched.calls.length, 0);
    assert.match(String(error.message), /Manual install/);
  } finally {
    rmSync(project, { recursive: true, force: true });
  }
});

test("resolveToolchainAsync downloads a missing versioned pin when allowed", async () => {
  const project = makeEmptyProject();
  const archive = makeToolchainArchive();
  const artifact = fakeArtifact(archive);
  const fetched = fakeFetch(archive.bytes);
  try {
    const resolution = await toolchain.resolveToolchainAsync({
      projectRoot: project,
      policy: { request: "zig@0.13.0" },
      env: {},
      allowDownload: true,
      artifacts: [artifact],
      fetchImpl: fetched.fetchImpl,
    });
    assert.equal(resolution.rung, "cache");
    assert.equal(fetched.calls.length, 1);
  } finally {
    rmSync(project, { recursive: true, force: true });
  }
});

test("resolveToolchainAsync never downloads for auto or bare pins", async () => {
  const project = makeEmptyProject();
  const fetched = fakeFetch(Buffer.from("unused"));
  try {
    await expectCodeAsync(
      toolchain.resolveToolchainAsync({
        projectRoot: project,
        policy: { request: "auto" },
        env: {},
        probe: fakeProbe(),
        allowDownload: true,
        fetchImpl: fetched.fetchImpl,
      }),
      "NS9102",
    );
    await expectCodeAsync(
      toolchain.resolveToolchainAsync({
        projectRoot: project,
        policy: { request: "zig" },
        env: {},
        probe: fakeProbe(),
        allowDownload: true,
        fetchImpl: fetched.fetchImpl,
      }),
      "NS9102",
    );
    assert.equal(fetched.calls.length, 0);
  } finally {
    rmSync(project, { recursive: true, force: true });
  }
});

test("resolveToolchainAsync refuses a download under --no-download", async () => {
  const project = makeEmptyProject();
  const archive = makeToolchainArchive();
  const artifact = fakeArtifact(archive);
  const fetched = fakeFetch(archive.bytes);
  try {
    const error = await expectCodeAsync(
      toolchain.resolveToolchainAsync({
        projectRoot: project,
        policy: { request: "zig@0.13.0" },
        env: {},
        probe: fakeProbe(),
        allowDownload: false,
        artifacts: [artifact],
        fetchImpl: fetched.fetchImpl,
      }),
      "NS9102",
    );
    assert.match(String(error.message), /download disabled/);
    assert.match(String(error.message), /Manual install/);
    assert.equal(fetched.calls.length, 0);
  } finally {
    rmSync(project, { recursive: true, force: true });
  }
});

test("resolveToolchainAsync reports manual instructions for an invalid cache marker", async () => {
  const project = makeEmptyProject();
  const cache = join(project, ".tools", "toolchain", "zig@0.13.0");
  mkdirSync(cache, { recursive: true });
  writeFileSync(
    join(cache, "toolchain.json"),
    JSON.stringify({ id: "zig@0.12.0", family: "zig", version: "0.12.0" }),
    "utf8",
  );
  writeFileSync(join(cache, process.platform === "win32" ? "zig.exe" : "zig"), "old", "utf8");
  const archive = makeToolchainArchive();
  const artifact = fakeArtifact(archive);
  const fetched = fakeFetch(archive.bytes);
  try {
    const error = await expectCodeAsync(
      toolchain.resolveToolchainAsync({
        projectRoot: project,
        policy: { request: "zig@0.13.0" },
        env: {},
        allowDownload: true,
        artifacts: [artifact],
        fetchImpl: fetched.fetchImpl,
      }),
      "NS9102",
    );
    assert.match(String(error.message), /Manual install/);
    assert.equal(fetched.calls.length, 0);
  } finally {
    rmSync(project, { recursive: true, force: true });
  }
});

test("doctorAsync downloads a missing pinned toolchain and reports it", async () => {
  const project = makeEmptyProject();
  const archive = makeToolchainArchive();
  const artifact = fakeArtifact(archive);
  const fetched = fakeFetch(archive.bytes);
  try {
    const result = await toolchain.doctorAsync(
      project,
      { request: "zig@0.13.0" },
      {},
      fakeProbe(),
      {
        allowDownload: true,
        artifacts: [artifact],
        fetchImpl: fetched.fetchImpl,
      },
    );
    assert.equal(result.winner?.rung, "cache");
    assert.equal(result.error, null);
    assert.ok(
      result.attempts.some((attempt) => attempt.detail.includes("downloaded")),
    );
  } finally {
    rmSync(project, { recursive: true, force: true });
  }
});

test("astra doctor --no-download refuses a missing pinned toolchain", () => {
  const project = makeProject("zig@0.13.0");
  try {
    const result = runNode([cli, "doctor", project, "--no-download"], {
      env: { ...process.env, ASTRA_CC: "" },
    });
    assert.notEqual(result.status, 0);
    const output = `${result.stdout}${result.stderr}`;
    assert.match(output, /NS9102/);
    assert.match(output, /download disabled/);
  } finally {
    rmSync(project, { recursive: true, force: true });
  }
});

test("astra build --no-download refuses a missing pinned toolchain", () => {
  const project = makeProject("zig@0.13.0");
  try {
    const result = runNode(
      [
        cli,
        "build",
        project,
        "--compiler",
        astraCompiler,
        "--target",
        "native",
        "--no-download",
      ],
      { env: { ...process.env, ASTRA_CC: "" } },
    );
    assert.notEqual(result.status, 0);
    const output = `${result.stdout}${result.stderr}`;
    assert.match(output, /NS9102/);
    assert.match(output, /download disabled/);
  } finally {
    rmSync(project, { recursive: true, force: true });
  }
});
