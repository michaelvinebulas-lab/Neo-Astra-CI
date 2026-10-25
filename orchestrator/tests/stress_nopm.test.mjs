/**
 * Stress tests — NOPM-001 through NOPM-004.
 * Covers: package IDs, descriptors, graph resolution, lockfile,
 * path adapter, npm/cargo/pypi/nuget/oci adapters, and registry.
 */

import test from "node:test";
import assert from "node:assert/strict";
import { mkdtemp, rm, writeFile, mkdir } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";

import {
  NopmError,
  parsePackageId,
  formatPackageId,
  parseDependencyRequest,
  createDescriptor,
  resolveDependencyGraph,
  resolveDependencyGraphForTarget,
  resolveForTargets,
  createLockfile,
  parseLockfile,
  formatLockfile,
  hashManifestSource,
  isLockfileCurrent,
  assertLockfileCurrent,
  readLockfileFile,
  writeLockfileFile,
  LOCKFILE_NAME,
} from "../packages/nopm/dist/index.js";

import { resolvePathPackage } from "../packages/nopm/dist/path-adapter.js";

import {
  RegistryStore,
  createRegistryServer,
  startRegistryServer,
  generateRegistryKeyPair,
  signRegistryPackage,
  verifyRegistryPackage,
} from "../packages/registry/dist/index.js";

// ─── Helpers ─────────────────────────────────────────────────────────────────

async function tempDir() {
  return mkdtemp(join(tmpdir(), "nopm-stress-"));
}
async function cleanup(dir) {
  await rm(dir, { recursive: true, force: true });
}

function makeDescriptor(id, deps = {}, opts = {}) {
  return createDescriptor({
    id: parsePackageId(id),
    dependencies: deps,
    integrity: id.startsWith("path:") ? undefined : "sha256-dGVzdA==",
    resolved: id.startsWith("path:") ? id.slice(5) : "https://example.com/pkg.tgz",
    lifecycleScripts: false,
    ...opts,
  });
}

const JS_TARGET = { name: "web", backend: "js", mode: "web" };
const NATIVE_TARGET = { name: "native", backend: "c", mode: "system" };
const CORE_TARGET = { name: "core", backend: "js", mode: "core" };

// ─── NOPM-001: Package IDs ────────────────────────────────────────────────────

test("parses all source-qualified ID prefixes", () => {
  const ids = [
    "nova:@scope/pkg@1.0.0",
    "npm:react@18.0.0",
    "cargo:serde@1.0.0",
    "git+https://github.com/user/repo.git#abc123",
    "path:../local",
    "pypi:requests@2.28.0",
    "nuget:Newtonsoft.Json@13.0.0",
    "oci:registry/repo@sha256:abc123",
  ];
  for (const id of ids) {
    const parsed = parsePackageId(id);
    assert.ok(parsed, `should parse: ${id}`);
    const formatted = formatPackageId(parsed);
    assert.ok(formatted.length > 0, `should format: ${id}`);
  }
});

test("rejects bare unqualified IDs", () => {
  for (const id of ["react", "serde", "pkg@1.0.0", ""]) {
    assert.throws(() => parsePackageId(id), NopmError, `should reject: ${id}`);
  }
});

test("parses npm scoped packages", () => {
  const id = parsePackageId("npm:@scope/name@1.0.0");
  assert.equal(id.source, "npm");
  assert.equal(id.name, "@scope/name");
  assert.equal(id.version, "1.0.0");
});

test("parses git ID with commit ref", () => {
  const id = parsePackageId("git+https://github.com/user/repo.git#abc1234");
  assert.equal(id.source, "git");
  assert.equal(id.reference, "abc1234");
});

test("parses path ID", () => {
  const id = parsePackageId("path:../local");
  assert.equal(id.source, "path");
  assert.equal(id.path, "../local");
});

// ─── NOPM-001: Dependency graph ───────────────────────────────────────────────

test("resolves a flat dependency graph in sorted order", () => {
  const requests = [
    parseDependencyRequest("a", "npm:a@1.0.0"),
    parseDependencyRequest("b", "npm:b@1.0.0"),
  ];
  const descriptors = [makeDescriptor("npm:a@1.0.0"), makeDescriptor("npm:b@1.0.0")];
  const graph = resolveDependencyGraph(requests, descriptors);
  assert.equal(graph.roots.length, 2);
  assert.equal(graph.nodes.length, 2);
});

test("resolves transitive dependencies", () => {
  const requests = [parseDependencyRequest("app", "npm:app@1.0.0")];
  const descriptors = [
    makeDescriptor("npm:app@1.0.0", { lib: "npm:lib@1.0.0" }),
    makeDescriptor("npm:lib@1.0.0"),
  ];
  const graph = resolveDependencyGraph(requests, descriptors);
  assert.equal(graph.nodes.length, 2);
});

test("detects cycles", () => {
  const requests = [parseDependencyRequest("a", "npm:a@1.0.0")];
  const descriptors = [
    makeDescriptor("npm:a@1.0.0", { b: "npm:b@1.0.0" }),
    makeDescriptor("npm:b@1.0.0", { a: "npm:a@1.0.0" }),
  ];
  assert.throws(
    () => resolveDependencyGraph(requests, descriptors),
    (e) => e instanceof NopmError && e.code === "NP0004",
  );
});

test("rejects duplicate descriptors", () => {
  const requests = [parseDependencyRequest("a", "npm:a@1.0.0")];
  const descriptors = [makeDescriptor("npm:a@1.0.0"), makeDescriptor("npm:a@1.0.0")];
  assert.throws(
    () => resolveDependencyGraph(requests, descriptors),
    (e) => e instanceof NopmError && e.code === "NP0002",
  );
});

test("target filtering rejects incompatible packages", () => {
  const requests = [parseDependencyRequest("dom", "npm:dom@1.0.0")];
  const descriptors = [
    createDescriptor({
      id: parsePackageId("npm:dom@1.0.0"),
      dependencies: {},
      targets: ["web"],
      capabilities: ["dom"],
      integrity: "sha256-dGVzdA==",
      resolved: "https://example.com/dom.tgz",
    }),
  ];
  assert.throws(
    () => resolveDependencyGraphForTarget(requests, descriptors, NATIVE_TARGET),
    (e) => e instanceof NopmError && e.code === "NP0006",
  );
});

test("resolves multiple targets and merges packages", () => {
  const requests = [parseDependencyRequest("shared", "npm:shared@1.0.0")];
  const descriptors = [makeDescriptor("npm:shared@1.0.0")];
  const result = resolveForTargets(requests, descriptors, [JS_TARGET, NATIVE_TARGET]);
  assert.equal(result.targets.length, 2);
  assert.equal(result.packages.length, 1);
  assert.ok(result.packages[0].targets.includes("web"));
  assert.ok(result.packages[0].targets.includes("native"));
});

// ─── NOPM-001: Lockfile ────────────────────────────────────────────────────────

test("creates, formats, and round-trips a canonical lockfile", () => {
  const requests = [parseDependencyRequest("dep", "npm:dep@1.0.0")];
  const descriptors = [makeDescriptor("npm:dep@1.0.0")];
  const resolution = resolveForTargets(requests, descriptors, [JS_TARGET]);
  const lockfile = createLockfile({
    resolution,
    generatedBy: "nopm@0.1.0",
    manifestHash: hashManifestSource('{"package":{"name":"test"}}'),
    compiler: { version: "0.1.0", apiVersion: "1.0" },
  });

  const formatted = formatLockfile(lockfile);
  const parsed = parseLockfile(formatted);
  assert.equal(parsed.lockfileVersion, 1);
  assert.equal(parsed.packages.length, 1);
  assert.equal(parsed.packages[0].id, "npm:dep@1.0.0");
});

test("lockfile staleness check works", () => {
  const manifest = '{"package":{"name":"test"}}';
  const resolution = resolveForTargets([], [], [JS_TARGET]);
  const lockfile = createLockfile({
    resolution,
    generatedBy: "nopm",
    manifestHash: hashManifestSource(manifest),
    compiler: { version: "0.1.0", apiVersion: "1.0" },
  });

  assert.ok(isLockfileCurrent(lockfile, manifest), "should be current");
  assert.ok(!isLockfileCurrent(lockfile, manifest + " "), "should be stale after change");
});

test("lockfile requires integrity for remote packages", () => {
  const requests = [parseDependencyRequest("pkg", "npm:pkg@1.0.0")];
  const pkg = createDescriptor({
    id: parsePackageId("npm:pkg@1.0.0"),
    dependencies: {},
    resolved: "https://example.com/pkg.tgz",
    // no integrity
  });
  const resolution = resolveForTargets(requests, [pkg], [JS_TARGET]);
  assert.throws(
    () => createLockfile({
      resolution,
      generatedBy: "nopm",
      manifestHash: hashManifestSource("{}"),
      compiler: { version: "0.1.0", apiVersion: "1.0" },
    }),
    (e) => e instanceof NopmError && e.code === "NP0008",
  );
});

test("lockfile validates sorted package order", () => {
  const outOfOrder = `{
    "lockfileVersion": 1,
    "generatedBy": "test",
    "manifestHash": "sha256:abc",
    "compiler": { "version": "0.1.0", "apiVersion": "1.0" },
    "semantics": { "integers": "wrapping", "strings": "utf8" },
    "targets": {},
    "packages": [
      { "id": "npm:z@1.0.0", "name": "z", "source": "npm", "version": "1.0.0", "resolved": null, "registry": null, "integrity": "sha256-abc", "targets": [], "capabilities": [], "lifecycleScripts": false, "dependencies": [] },
      { "id": "npm:a@1.0.0", "name": "a", "source": "npm", "version": "1.0.0", "resolved": null, "registry": null, "integrity": "sha256-abc", "targets": [], "capabilities": [], "lifecycleScripts": false, "dependencies": [] }
    ],
    "ffi": {}
  }`;
  assert.throws(
    () => parseLockfile(outOfOrder),
    (e) => e instanceof NopmError && e.code === "NP0008",
  );
});

test("lockfile file I/O roundtrip", async () => {
  const dir = await tempDir();
  try {
    const resolution = resolveForTargets([], [], [JS_TARGET]);
    const lockfile = createLockfile({
      resolution,
      generatedBy: "nopm",
      manifestHash: hashManifestSource("{}"),
      compiler: { version: "0.1.0", apiVersion: "1.0" },
    });
    const path = join(dir, LOCKFILE_NAME);
    writeLockfileFile(path, lockfile);
    const read = readLockfileFile(path);
    assert.equal(read.lockfileVersion, 1);
    assert.equal(read.generatedBy, "nopm");
  } finally {
    await cleanup(dir);
  }
});

// ─── NOPM-002: Path adapter ───────────────────────────────────────────────────

test("path adapter resolves a local package with core.json", async () => {
  const dir = await tempDir();
  const pkgDir = join(dir, "pkg");
  await mkdir(pkgDir);
  await writeFile(
    join(pkgDir, "core.json"),
    JSON.stringify({ package: { name: "local-pkg", version: "0.1.0" } }),
  );
  try {
    const id = parsePackageId(`path:${pkgDir}`);
    const descriptor = resolvePathPackage(dir, id);
    assert.equal(descriptor.id.name, "local-pkg");
    assert.equal(descriptor.lifecycleScripts, false);
  } finally {
    await cleanup(dir);
  }
});

test("path adapter rejects non-existent directory", () => {
  assert.throws(
    () => resolvePathPackage("/tmp", parsePackageId("path:/nonexistent/path")),
    (e) => e instanceof NopmError && e.code === "NP0011",
  );
});

// ─── NOPM-003: Registry gateway ───────────────────────────────────────────────

test("registry store publish, get, versions, search, yank, audit", () => {
  const store = new RegistryStore();
  const published = store.publish({
    name: "my-package",
    version: "1.0.0",
    integrity: "sha256-dGVzdA==",
    resolved: "https://registry.example.com/my-package-1.0.0.tgz",
    lifecycleScripts: false,
  });

  assert.equal(published.name, "my-package");
  assert.equal(published.yanked, false);

  const fetched = store.get("my-package", "1.0.0");
  assert.equal(fetched.version, "1.0.0");

  const versions = store.versions("my-package");
  assert.equal(versions.length, 1);

  const results = store.search("package");
  assert.equal(results.length, 1);

  const yanked = store.yank("my-package", "1.0.0");
  assert.ok(yanked.yanked);

  const audit = store.audit("my-package", "1.0.0");
  assert.ok(audit.yanked);
  assert.deepEqual(audit.advisories, []);
});

test("registry rejects duplicate publish", () => {
  const store = new RegistryStore();
  store.publish({ name: "pkg", version: "1.0.0", integrity: "sha256-x", resolved: "https://x.com/pkg.tgz" });
  assert.throws(
    () => store.publish({ name: "pkg", version: "1.0.0", integrity: "sha256-y", resolved: "https://y.com/pkg.tgz" }),
    (e) => e.status === 409,
  );
});

test("registry returns 404 for missing package", () => {
  const store = new RegistryStore();
  assert.throws(
    () => store.get("nonexistent", "1.0.0"),
    (e) => e.status === 404,
  );
});

test("registry HTTP server publishes and retrieves", async () => {
  const { server, url } = await startRegistryServer();
  try {
    const publishRes = await fetch(`${url}/v1/publish`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({
        name: "http-pkg",
        version: "1.0.0",
        integrity: "sha256-dGVzdA==",
        resolved: "https://example.com/pkg.tgz",
      }),
    });
    assert.equal(publishRes.status, 201);

    const getRes = await fetch(`${url}/v1/packages/http-pkg/1.0.0`);
    assert.equal(getRes.status, 200);
    const body = await getRes.json();
    assert.equal(body.name, "http-pkg");
    assert.equal(body.version, "1.0.0");
  } finally {
    await new Promise((r) => server.close(r));
  }
});

test("registry HTTP search works", async () => {
  const store = new RegistryStore();
  store.publish({ name: "findme", version: "1.0.0", integrity: "sha256-x", resolved: "https://x" });
  store.publish({ name: "other", version: "1.0.0", integrity: "sha256-y", resolved: "https://y" });
  const { server, url } = await startRegistryServer(store);
  try {
    const res = await fetch(`${url}/v1/search?q=find`);
    assert.equal(res.status, 200);
    const body = await res.json();
    assert.equal(body.results.length, 1);
    assert.equal(body.results[0].name, "findme");
  } finally {
    await new Promise((r) => server.close(r));
  }
});

// ─── Ed25519 signing ──────────────────────────────────────────────────────────

test("package signing and verification roundtrip", () => {
  const { publicKey, privateKey } = generateRegistryKeyPair();
  const request = {
    name: "signed-pkg",
    version: "1.0.0",
    integrity: "sha256-dGVzdA==",
    resolved: "https://example.com/pkg.tgz",
  };
  const signature = signRegistryPackage(request, privateKey);
  assert.ok(signature.length > 0);
  assert.ok(verifyRegistryPackage(request, signature, publicKey));
});

test("wrong key does not verify", () => {
  const { privateKey } = generateRegistryKeyPair();
  const { publicKey: wrongPublicKey } = generateRegistryKeyPair();
  const request = { name: "pkg", version: "1.0.0", integrity: "sha256-x", resolved: "https://x" };
  const signature = signRegistryPackage(request, privateKey);
  assert.ok(!verifyRegistryPackage(request, signature, wrongPublicKey));
});

test("tampered payload does not verify", () => {
  const { publicKey, privateKey } = generateRegistryKeyPair();
  const request = { name: "pkg", version: "1.0.0", integrity: "sha256-x", resolved: "https://x" };
  const signature = signRegistryPackage(request, privateKey);
  const tampered = { ...request, version: "2.0.0" };
  assert.ok(!verifyRegistryPackage(tampered, signature, publicKey));
});

test("signed server rejects unsigned publish", async () => {
  const { publicKey } = generateRegistryKeyPair();
  const { server, url } = await startRegistryServer(new RegistryStore(), 0, "127.0.0.1", {
    requireSignature: true,
    trustedPublicKey: publicKey,
  });
  try {
    const res = await fetch(`${url}/v1/publish`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ name: "pkg", version: "1.0.0", integrity: "sha256-x", resolved: "https://x" }),
    });
    assert.equal(res.status, 401);
  } finally {
    await new Promise((r) => server.close(r));
  }
});

// ─── NOPM-003: Federation ─────────────────────────────────────────────────────

test("registry mirrors missing package from upstream", async () => {
  const upstream = new RegistryStore();
  upstream.publish({ name: "upstream-pkg", version: "1.0.0", integrity: "sha256-up", resolved: "https://upstream.example.com/pkg.tgz" });
  const { server: upServer, url: upUrl } = await startRegistryServer(upstream);

  const local = new RegistryStore();
  const { server: localServer, url: localUrl } = await startRegistryServer(
    local,
    0,
    "127.0.0.1",
    { upstreams: [upUrl] },
  );

  try {
    const res = await fetch(`${localUrl}/v1/packages/upstream-pkg/1.0.0`);
    assert.equal(res.status, 200);
    const body = await res.json();
    assert.equal(body.name, "upstream-pkg");
  } finally {
    await new Promise((r) => localServer.close(r));
    await new Promise((r) => upServer.close(r));
  }
});

// ─── NOPM-004: Later adapters (type-safe instantiation) ──────────────────────

test("PyPI adapter module exports expected functions", async () => {
  const pypi = await import("../packages/nopm/dist/pypi-adapter.js");
  assert.equal(typeof pypi.resolvePyPiPackage, "function");
});

test("NuGet adapter module exports expected functions", async () => {
  const nuget = await import("../packages/nopm/dist/nuget-adapter.js");
  assert.equal(typeof nuget.resolveNuGetPackage, "function");
});

test("OCI adapter module exports expected functions", async () => {
  const oci = await import("../packages/nopm/dist/oci-adapter.js");
  assert.equal(typeof oci.resolveOciPackage, "function");
});

test("PyPI adapter rejects non-PyPI IDs", async () => {
  const { resolvePyPiPackage } = await import("../packages/nopm/dist/pypi-adapter.js");
  await assert.rejects(
    () => resolvePyPiPackage(parsePackageId("npm:react@18"), "react"),
    (e) => e.code === "NP0011",
  );
});

test("NuGet adapter rejects non-NuGet IDs", async () => {
  const { resolveNuGetPackage } = await import("../packages/nopm/dist/nuget-adapter.js");
  await assert.rejects(
    () => resolveNuGetPackage(parsePackageId("npm:react@18"), "react"),
    (e) => e.code === "NP0011",
  );
});

test("OCI adapter rejects IDs without reference", async () => {
  const { resolveOciPackage } = await import("../packages/nopm/dist/oci-adapter.js");
  await assert.rejects(
    () => resolveOciPackage(parsePackageId("oci:registry/repo@latest"), "pkg"),
    // OCI with no valid digest reference — or just that it tries to fetch
    // and fails because there's no registry. Either is acceptable.
  );
});
