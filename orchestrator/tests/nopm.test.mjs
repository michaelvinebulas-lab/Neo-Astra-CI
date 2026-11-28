import assert from "node:assert/strict";
import { execFileSync, spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import { existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";

import {
  NopmError,
  assertLockfileCurrent,
  createDescriptor,
  createLockfile,
  formatLockfile,
  formatPackageId,
  hashManifestSource,
  isLockfileCurrent,
  parseDependencyRequest,
  parseLockfile,
  parsePackageId,
  readLockfileFile,
  resolveDependencyGraph,
  resolveDependencyGraphForTarget,
  resolveForTargets,
  validateLockfile,
  writeLockfileFile,
} from "../packages/nopm/dist/index.js";
import { resolveNpmPackage } from "../packages/nopm/dist/npm-adapter.js";
import { commandName, defaultCommandRunner } from "../packages/nopm/dist/command.js";
import { resolveGitPackage } from "../packages/nopm/dist/git-adapter.js";
import { resolveCargoPackage } from "../packages/nopm/dist/cargo-adapter.js";
import { resolvePyPiPackage } from "../packages/nopm/dist/pypi-adapter.js";
import { resolveNuGetPackage } from "../packages/nopm/dist/nuget-adapter.js";
import { resolveOciPackage } from "../packages/nopm/dist/oci-adapter.js";
import { resolveRegistryPackage } from "../packages/nopm/dist/registry-client.js";
import { installLockedPackages } from "../packages/nopm/dist/store.js";
import { detectManagers, executeSysInstall, planSysInstall } from "../packages/nopm/dist/sys-adapter.js";

function expectCode(fn, code) {
  assert.throws(fn, (error) => {
    assert.ok(error instanceof NopmError, `expected NopmError, got ${String(error)}`);
    assert.equal(error.code, code);
    return true;
  });
}

test("parses and formats every package source", () => {
  const cases = [
    ["nova:@scope/name@1.2.3", "nova:@scope/name@1.2.3"],
    ["npm:react@19", "npm:react@19"],
    ["npm:@scope/name@19.0.0", "npm:@scope/name@19.0.0"],
    ["npm:react", "npm:react"],
    ["npm:react@next", "npm:react@next"],
    ["cargo:serde@1", "cargo:serde@1"],
    ["cargo:serde@>=1 <2", "cargo:serde@>=1 <2"],
    ["git+https://example.com/repo.git#v1.2.0", "git+https://example.com/repo.git#v1.2.0"],
    ["git+ssh://git@example.com/repo#abc123", "git+ssh://git@example.com/repo#abc123"],
    ["path:../local-package", "path:../local-package"],
    ["pypi:requests@2.32.3", "pypi:requests@2.32.3"],
    ["pypi:requests", "pypi:requests"],
    ["nuget:Newtonsoft.Json@13.0.3", "nuget:Newtonsoft.Json@13.0.3"],
    ["oci:ghcr.io/org/image@sha256:abc123", "oci:ghcr.io/org/image@sha256:abc123"],
  ];

  for (const [input, expected] of cases) {
    const id = parsePackageId(input);
    assert.equal(formatPackageId(id), expected);
    assert.equal(formatPackageId(parsePackageId(formatPackageId(id))), expected);
    assert.equal(id.raw, input);
  }
});

test("exposes structured package fields", () => {
  const npm = parsePackageId("npm:@scope/name@19");
  assert.equal(npm.source, "npm");
  assert.equal(npm.name, "@scope/name");
  assert.equal(npm.version, "19");
  assert.equal(npm.reference, null);
  assert.equal(npm.path, null);

  const git = parsePackageId("git+https://example.com/repo.git#main");
  assert.equal(git.source, "git");
  assert.equal(git.name, "https://example.com/repo.git");
  assert.equal(git.reference, "main");

  const path = parsePackageId("path:../local-package");
  assert.equal(path.source, "path");
  assert.equal(path.name, "local-package");
  assert.equal(path.path, "../local-package");

  const oci = parsePackageId("oci:ghcr.io/org/image@sha256:abc123");
  assert.equal(oci.source, "oci");
  assert.equal(oci.name, "ghcr.io/org/image");
  assert.equal(oci.reference, "sha256:abc123");
});

test("rejects malformed package IDs", () => {
  expectCode(() => parsePackageId("react"), "NP0001");
  expectCode(() => parsePackageId("pypi:"), "NP0001");
  expectCode(() => parsePackageId("nuget:"), "NP0001");
  expectCode(() => parsePackageId("oci:ghcr.io/org/image"), "NP0001");
  expectCode(() => parsePackageId("nova:react"), "NP0001");
  expectCode(() => parsePackageId("nova:react@19"), "NP0001");
  expectCode(() => parsePackageId("nova:react@v1.2.3"), "NP0001");
  expectCode(() => parsePackageId("npm:@scope"), "NP0001");
  expectCode(() => parsePackageId("npm:react@"), "NP0001");
  expectCode(() => parsePackageId("cargo:serde@"), "NP0001");
  expectCode(() => parsePackageId("git+https://example.com/repo.git"), "NP0001");
  expectCode(() => parsePackageId("git+file:///repo#main"), "NP0001");
  expectCode(() => parsePackageId("path:"), "NP0001");
  expectCode(() => parsePackageId(" path:../x"), "NP0001");
});

test("parses dependency aliases and rejects invalid declarations", () => {
  const dependency = parseDependencyRequest("react", "npm:react@19");
  assert.equal(dependency.alias, "react");
  assert.equal(formatPackageId(dependency.id), "npm:react@19");

  expectCode(() => parseDependencyRequest("bad alias", "npm:react@19"), "NP0002");
  expectCode(() => parseDependencyRequest("react", "react"), "NP0002");
});

test("creates frozen descriptors", () => {
  const descriptor = createDescriptor({
    id: parsePackageId("nova:app@1.0.0"),
    dependencies: { lib: "nova:lib@1.0.0" },
    targets: ["web"],
    capabilities: ["dom"],
    integrity: "sha256-abc",
    metadata: { license: "MIT" },
  });

  assert.ok(Object.isFrozen(descriptor));
  assert.ok(Object.isFrozen(descriptor.dependencies));
  assert.ok(Object.isFrozen(descriptor.targets));
  assert.ok(Object.isFrozen(descriptor.capabilities));
  assert.equal(descriptor.dependencies.lib, "nova:lib@1.0.0");
});

test("resolves a deterministic dependency graph", () => {
  const descriptors = [
    createDescriptor({
      id: parsePackageId("nova:app@1.0.0"),
      dependencies: {
        zeta: "nova:zeta@1.0.0",
        alpha: "nova:alpha@1.0.0",
      },
    }),
    createDescriptor({
      id: parsePackageId("nova:zeta@1.0.0"),
      dependencies: { shared: "nova:shared@1.0.0" },
    }),
    createDescriptor({
      id: parsePackageId("nova:alpha@1.0.0"),
      dependencies: { shared: "nova:shared@1.0.0" },
    }),
    createDescriptor({ id: parsePackageId("nova:shared@1.0.0") }),
  ];

  const graph = resolveDependencyGraph(
    [parseDependencyRequest("app", "nova:app@1.0.0")],
    descriptors,
  );

  assert.equal(graph.roots.length, 1);
  assert.deepEqual(
    graph.roots[0].dependencies.map((node) => formatPackageId(node.id)),
    ["nova:alpha@1.0.0", "nova:zeta@1.0.0"],
  );
  assert.deepEqual(
    graph.nodes.map((node) => formatPackageId(node.id)),
    [
      "nova:alpha@1.0.0",
      "nova:app@1.0.0",
      "nova:shared@1.0.0",
      "nova:zeta@1.0.0",
    ],
  );
  assert.ok(Object.isFrozen(graph));
  assert.ok(Object.isFrozen(graph.roots));
  assert.ok(Object.isFrozen(graph.nodes));
  assert.ok(Object.isFrozen(graph.roots[0].dependencies));
});

test("detects dependency cycles", () => {
  const descriptors = [
    createDescriptor({
      id: parsePackageId("nova:a@1.0.0"),
      dependencies: { b: "nova:b@1.0.0" },
    }),
    createDescriptor({
      id: parsePackageId("nova:b@1.0.0"),
      dependencies: { a: "nova:a@1.0.0" },
    }),
  ];

  expectCode(
    () =>
      resolveDependencyGraph(
        [parseDependencyRequest("a", "nova:a@1.0.0")],
        descriptors,
      ),
    "NP0004",
  );
});

test("reports missing descriptors and duplicate descriptors", () => {
  expectCode(
    () =>
      resolveDependencyGraph(
        [parseDependencyRequest("missing", "nova:missing@1.0.0")],
        [],
      ),
    "NP0003",
  );

  const descriptor = createDescriptor({ id: parsePackageId("nova:a@1.0.0") });
  expectCode(
    () =>
      resolveDependencyGraph(
        [parseDependencyRequest("a", "nova:a@1.0.0")],
        [descriptor, descriptor],
      ),
    "NP0002",
  );
});

test("rejects conflicting aliases at the root", () => {
  expectCode(
    () =>
      resolveDependencyGraph(
        [
          parseDependencyRequest("lib", "nova:a@1.0.0"),
          parseDependencyRequest("lib", "nova:b@1.0.0"),
        ],
        [
          createDescriptor({ id: parsePackageId("nova:a@1.0.0") }),
          createDescriptor({ id: parsePackageId("nova:b@1.0.0") }),
        ],
      ),
    "NP0005",
  );
});

// ─── target and mode filtering ──────────────────────────────────────────────

const webTarget = { name: "web", backend: "js", mode: "web" };
const nativeTarget = { name: "native", backend: "c", mode: "system" };

test("filters packages by target name, backend, or triple", () => {
  const webOnly = createDescriptor({
    id: parsePackageId("nova:web-only@1.0.0"),
    targets: ["web"],
  });
  const tsOnly = createDescriptor({
    id: parsePackageId("npm:ts-only@1.0.0"),
    targets: ["typescript"],
  });

  assert.doesNotThrow(() =>
    resolveDependencyGraphForTarget(
      [parseDependencyRequest("web", "nova:web-only@1.0.0")],
      [webOnly],
      webTarget,
    ),
  );
  expectCode(
    () =>
      resolveDependencyGraphForTarget(
        [parseDependencyRequest("web", "nova:web-only@1.0.0")],
        [webOnly],
        nativeTarget,
      ),
    "NP0006",
  );

  assert.doesNotThrow(() =>
    resolveDependencyGraphForTarget(
      [parseDependencyRequest("ts", "npm:ts-only@1.0.0")],
      [tsOnly],
      { name: "web", backend: "typescript", mode: "web" },
    ),
  );
});

test("enforces capability-derived modes per target", () => {
  const app = createDescriptor({
    id: parsePackageId("nova:app-lib@1.0.0"),
    capabilities: ["console"],
  });
  const system = createDescriptor({
    id: parsePackageId("nova:system-lib@1.0.0"),
    capabilities: ["fs"],
  });
  const impossible = createDescriptor({
    id: parsePackageId("nova:impossible@1.0.0"),
    capabilities: ["dom", "fs"],
  });

  assert.doesNotThrow(() =>
    resolveDependencyGraphForTarget(
      [parseDependencyRequest("app", "nova:app-lib@1.0.0")],
      [app],
      { name: "web", backend: "js", mode: "web" },
    ),
  );
  expectCode(
    () =>
      resolveDependencyGraphForTarget(
        [parseDependencyRequest("app", "nova:app-lib@1.0.0")],
        [app],
        { name: "core", backend: "js", mode: "core" },
      ),
    "NP0007",
  );
  expectCode(
    () =>
      resolveDependencyGraphForTarget(
        [parseDependencyRequest("system", "nova:system-lib@1.0.0")],
        [system],
        webTarget,
      ),
    "NP0007",
  );
  expectCode(
    () =>
      resolveDependencyGraphForTarget(
        [parseDependencyRequest("impossible", "nova:impossible@1.0.0")],
        [impossible],
        webTarget,
      ),
    "NP0007",
  );
});

test("resolves multiple targets and records the package union", () => {
  const universal = createDescriptor({
    id: parsePackageId("nova:universal@1.0.0"),
    capabilities: ["console"],
    integrity: "sha256-universal",
  });

  const resolution = resolveForTargets(
    [parseDependencyRequest("universal", "nova:universal@1.0.0")],
    [universal],
    [
      { name: "web", backend: "js", mode: "web" },
      { name: "app", backend: "js", mode: "app" },
    ],
  );

  assert.deepEqual(
    resolution.targets.map((entry) => entry.target.name),
    ["app", "web"],
  );
  const entry = resolution.packages.find(
    (item) => item.id === "nova:universal@1.0.0",
  );
  assert.deepEqual(entry.targets, ["app", "web"]);
});

// ─── lockfile model ─────────────────────────────────────────────────────────

function sampleResolution() {
  const app = createDescriptor({
    id: parsePackageId("nova:app@1.0.0"),
    dependencies: {
      lib: "npm:lib@2.0.0",
      local: "path:../local",
    },
    integrity: "sha256-app",
    resolved: "https://registry.example/app-1.0.0.tgz",
    registry: "https://registry.neovastra.dev",
    capabilities: ["console"],
    lifecycleScripts: false,
  });
  const lib = createDescriptor({
    id: parsePackageId("npm:lib@2.0.0"),
    targets: ["web"],
    integrity: "sha512-lib",
    resolved: "https://registry.npmjs.org/lib/-/lib-2.0.0.tgz",
    registry: "https://registry.npmjs.org",
  });
  const local = createDescriptor({
    id: parsePackageId("path:../local"),
    targets: ["web"],
  });

  return resolveForTargets(
    [parseDependencyRequest("app", "nova:app@1.0.0")],
    [app, lib, local],
    [webTarget],
  );
}

test("creates, formats, and parses a canonical lockfile", () => {
  const lockfile = createLockfile({
    resolution: sampleResolution(),
    generatedBy: "astra/0.1.0",
    manifestHash: "sha256-manifest",
    compiler: { version: "0.1.0", apiVersion: "1.0" },
    ffi: {
      zeta: { b: 2 },
      alpha: { a: 1 },
    },
  });

  const formatted = formatLockfile(lockfile);
  assert.ok(formatted.endsWith("\n"));
  assert.ok(formatted.includes('"lockfileVersion": 1'));
  assert.ok(formatted.indexOf('"alpha"') < formatted.indexOf('"zeta"'));
  assert.ok(
    formatted.indexOf('"id": "nova:app@1.0.0"') <
      formatted.indexOf('"id": "npm:lib@2.0.0"'),
  );

  const parsed = parseLockfile(formatted);
  assert.deepEqual(parsed, lockfile);
  assert.equal(formatLockfile(parsed), formatted);
  assert.equal(validateLockfile(JSON.parse(formatted)).lockfileVersion, 1);
});

test("requires integrity for remote packages in a lockfile", () => {
  const app = createDescriptor({
    id: parsePackageId("nova:app@1.0.0"),
  });
  const resolution = resolveForTargets(
    [parseDependencyRequest("app", "nova:app@1.0.0")],
    [app],
    [webTarget],
  );

  expectCode(
    () =>
      createLockfile({
        resolution,
        generatedBy: "astra/0.1.0",
        manifestHash: "sha256-manifest",
        compiler: { version: "0.1.0", apiVersion: "1.0" },
      }),
    "NP0008",
  );
});

test("rejects invalid lockfile documents", () => {
  expectCode(() => parseLockfile("not json"), "NP0008");
  expectCode(
    () =>
      validateLockfile({
        lockfileVersion: 2,
        generatedBy: "astra/0.1.0",
        manifestHash: "sha256-manifest",
        compiler: { version: "0.1.0", apiVersion: "1.0" },
        semantics: { integers: "wrapping", strings: "utf8" },
        targets: {},
        packages: [],
        ffi: {},
      }),
    "NP0008",
  );
});

test("writes, reads, and staleness-checks a lockfile file", () => {
  const directory = mkdtempSync(join(tmpdir(), "nopm-lock-"));
  try {
    const manifestSource = '{"package":{"name":"demo"}}\n';
    const manifestHash = hashManifestSource(manifestSource);
    const lockfile = createLockfile({
      resolution: sampleResolution(),
      generatedBy: "astra/0.1.0",
      manifestHash,
      compiler: { version: "0.1.0", apiVersion: "1.0" },
    });
    const path = join(directory, "core.lock.json");

    writeLockfileFile(path, lockfile);
    const onDisk = readFileSync(path, "utf8");
    assert.equal(onDisk, formatLockfile(lockfile));

    const parsed = readLockfileFile(path);
    assert.deepEqual(parsed, lockfile);
    assert.equal(isLockfileCurrent(parsed, manifestSource), true);
    assert.equal(isLockfileCurrent(parsed, `${manifestSource} `), false);
    assertLockfileCurrent(parsed, manifestSource);

    expectCode(() => assertLockfileCurrent(parsed, `${manifestSource} `), "NP0009");
  } finally {
    rmSync(directory, { recursive: true, force: true });
  }
});

test("reports missing lockfiles", () => {
  const directory = mkdtempSync(join(tmpdir(), "nopm-missing-"));
  try {
    expectCode(() => readLockfileFile(join(directory, "core.lock.json")), "NP0009");
  } finally {
    rmSync(directory, { recursive: true, force: true });
  }
});

// ─── adapters and CLI ───────────────────────────────────────────────────────

test("default command runner executes npm on every host", () => {
  const result = defaultCommandRunner.run(commandName("npm"), ["--version"]);
  assert.equal(result.status, 0, result.stderr);
  assert.match(result.stdout.trim(), /^\d+\.\d+\.\d+/);
});

test("npm adapter resolves an exact version and metadata", () => {  const runner = {
    run(_command, args) {
      if (args[0] === "view" && args[2] === "version") {
        return { status: 0, stdout: '"19.1.0"\n', stderr: "" };
      }
      return {
        status: 0,
        stdout: `${JSON.stringify({
          name: "react",
          version: "19.1.0",
          dependencies: { scheduler: "^0.23.0" },
          dist: {
            tarball: "https://registry.npmjs.org/react/-/react-19.1.0.tgz",
            integrity: "sha512-react",
          },
        })}\n`,
        stderr: "",
      };
    },
  };

  const resolution = resolveNpmPackage(parsePackageId("npm:react@19"), "react", runner);
  assert.equal(formatPackageId(resolution.descriptor.id), "npm:react@19.1.0");
  assert.equal(resolution.descriptor.integrity, "sha512-react");
  assert.equal(
    resolution.descriptor.resolved,
    "https://registry.npmjs.org/react/-/react-19.1.0.tgz",
  );
  assert.deepEqual(resolution.descriptor.dependencies, { scheduler: "^0.23.0" });
  assert.equal(formatPackageId(resolution.request.id), "npm:react@19.1.0");
});

test("npm adapter reports command failures", () => {
  const runner = {
    run() {
      return { status: 1, stdout: "", stderr: "registry unavailable" };
    },
  };
  expectCode(
    () => resolveNpmPackage(parsePackageId("npm:react@19"), "react", runner),
    "NP0011",
  );
});

test("nopm lock and install work for a path package", () => {
  const cliPath = fileURLToPath(new URL("../packages/nopm/dist/cli.js", import.meta.url));
  const directory = mkdtempSync(join(tmpdir(), "nopm-cli-"));
  try {
    const packageDirectory = join(directory, "local-utils");
    writeFileSync(
      join(directory, "core.json"),
      JSON.stringify(
        {
          package: { name: "demo", version: "0.1.0" },
          dependencies: {
            "local-utils": { source: "path", path: "local-utils" },
          },
          packageSources: { path: true },
        },
        null,
        2,
      ),
    );
    writeFileSync(
      join(directory, "build.json"),
      JSON.stringify(
        {
          targets: { web: { backend: "js", mode: "web" } },
        },
        null,
        2,
      ),
    );
    mkdirSync(packageDirectory, { recursive: true });
    writeFileSync(
      join(packageDirectory, "core.json"),
      JSON.stringify(
        {
          package: { name: "local-utils", version: "0.1.0" },
        },
        null,
        2,
      ),
    );
    writeFileSync(
      join(packageDirectory, "build.json"),
      JSON.stringify(
        {
          targets: { web: { backend: "js", mode: "web" } },
        },
        null,
        2,
      ),
    );

    execFileSync(process.execPath, [cliPath, "lock", directory], { encoding: "utf8" });
    const lockfile = readLockfileFile(join(directory, "core.lock.json"));
    assert.equal(lockfile.packages.length, 1);
    assert.equal(lockfile.packages[0].id, "path:local-utils");
    assert.equal(lockfile.packages[0].resolved, "local-utils");

    execFileSync(process.execPath, [cliPath, "install", directory, "--locked"], {
      encoding: "utf8",
    });
    const store = JSON.parse(
      readFileSync(join(directory, ".astra", "store", "index.json"), "utf8"),
    );
    assert.equal(store.packages.length, 1);
  } finally {
    rmSync(directory, { recursive: true, force: true });
  }
});

test("git adapter resolves a cached pinned commit", () => {
  const commit = "0123456789abcdef0123456789abcdef01234567";
  const cacheRoot = mkdtempSync(join(tmpdir(), "nopm-git-"));
  try {
    const packageDirectory = join(cacheRoot, "git", commit);
    mkdirSync(packageDirectory, { recursive: true });
    writeFileSync(
      join(packageDirectory, "core.json"),
      JSON.stringify({
        package: { name: "git-pkg", version: "1.0.0" },
      }),
    );
    const runner = {
      run(_command, args) {
        assert.deepEqual(args.slice(0, 2), ["ls-remote", "https://example.com/repo.git"]);
        return {
          status: 0,
          stdout: `${commit}\trefs/tags/v1\n`,
          stderr: "",
        };
      },
    };

    const resolution = resolveGitPackage(
      parsePackageId("git+https://example.com/repo.git#v1"),
      "repo",
      cacheRoot,
      runner,
    );
    assert.equal(formatPackageId(resolution.descriptor.id), "git+https://example.com/repo.git#v1");
    assert.equal(resolution.descriptor.commit, commit);
    assert.equal(resolution.descriptor.integrity, `git:${commit}`);
  } finally {
    rmSync(cacheRoot, { recursive: true, force: true });
  }
});

test("cargo adapter resolves exact versions and normal dependencies", async () => {
  const fetchImpl = async (url) => {
    if (url.endsWith("/dependencies")) {
      return jsonResponse({
        dependencies: [
          { crate_id: "serde_derive", req: "1.0.200", kind: "normal", optional: false },
          { crate_id: "dev-only", req: "1", kind: "dev", optional: false },
          { crate_id: "optional", req: "1", kind: "normal", optional: true },
        ],
      });
    }
    if (url.endsWith("/serde")) {
      return jsonResponse({
        crate: { max_stable_version: "1.0.200" },
        versions: [
          { num: "1.0.200", yanked: false },
          { num: "1.1.0", yanked: false },
          { num: "2.0.0", yanked: true },
        ],
      });
    }
    return jsonResponse({
      version: { num: "1.0.200", checksum: "abc123", yanked: false },
    });
  };

  const resolution = await resolveCargoPackage(
    parsePackageId("cargo:serde"),
    "serde",
    { fetchImpl },
  );
  assert.equal(formatPackageId(resolution.descriptor.id), "cargo:serde@1.0.200");
  assert.equal(resolution.descriptor.integrity, "sha256:abc123");
  assert.deepEqual(resolution.descriptor.dependencies, { serde_derive: "1.0.200" });

  const rangeResolution = await resolveCargoPackage(
    parsePackageId("cargo:serde@^1.0"),
    "serde",
    {
      fetchImpl: async (url) => {
        if (url.endsWith("/serde")) {
          return jsonResponse({
            versions: [
              { num: "1.0.200", yanked: false },
              { num: "1.1.0", yanked: false },
              { num: "2.0.0", yanked: false },
            ],
          });
        }
        if (url.endsWith("/dependencies")) {
          return jsonResponse({ dependencies: [] });
        }
        return jsonResponse({
          version: { num: "1.1.0", checksum: "def456", yanked: false },
        });
      },
    },
  );
  assert.equal(formatPackageId(rangeResolution.descriptor.id), "cargo:serde@1.1.0");

  await assert.rejects(
    () =>
      resolveCargoPackage(parsePackageId("cargo:serde@9"), "serde", {
        fetchImpl: async () => jsonResponse({ versions: [{ num: "1.0.200", yanked: false }] }),
      }),
    (error) => error instanceof NopmError && error.code === "NP0011",
  );
});

test("registry adapter reads a Nova package descriptor", async () => {
  const fetchImpl = async () =>
    jsonResponse({
      name: "@scope/pkg",
      version: "1.2.3",
      dependencies: { lib: "npm:lib@2" },
      targets: ["web"],
      capabilities: ["dom"],
      integrity: "sha512-pkg",
      resolved: "https://registry.example/pkg.tgz",
    });

  const resolution = await resolveRegistryPackage(
    parsePackageId("nova:@scope/pkg@1.2.3"),
    "pkg",
    { registry: "https://registry.example", fetchImpl },
  );
  assert.equal(formatPackageId(resolution.descriptor.id), "nova:@scope/pkg@1.2.3");
  assert.equal(resolution.descriptor.integrity, "sha512-pkg");
  assert.deepEqual(resolution.descriptor.dependencies, { lib: "npm:lib@2" });
});

test("PyPI adapter resolves an sdist and exact dependencies", async () => {
  const fetchImpl = async () =>
    jsonResponse({
      info: {
        name: "requests",
        version: "2.32.3",
        requires_dist: ["urllib3==2.0.7", "charset-normalizer>=3"],
      },
      urls: [
        {
          packagetype: "sdist",
          url: "https://files.pythonhosted.org/requests-2.32.3.tar.gz",
          digests: { sha256: "abc123" },
        },
      ],
    });

  const resolution = await resolvePyPiPackage(
    parsePackageId("pypi:requests@2.32.3"),
    "requests",
    { fetchImpl },
  );
  assert.equal(formatPackageId(resolution.descriptor.id), "pypi:requests@2.32.3");
  assert.equal(resolution.descriptor.integrity, "sha256:abc123");
  assert.deepEqual(resolution.descriptor.dependencies, {
    urllib3: "pypi:urllib3@2.0.7",
  });
});

test("NuGet adapter resolves integrity and exact dependency ranges", async () => {
  const fetchImpl = async (url) => {
    if (url.endsWith(".sha512")) {
      return {
        ok: true,
        status: 200,
        text: async () => "base64hash",
      };
    }
    return jsonResponse({
      catalogEntry: {
        dependencyGroups: [
          { dependencies: [{ id: "Lib", range: "[1.0.0]" }] },
        ],
      },
    });
  };

  const resolution = await resolveNuGetPackage(
    parsePackageId("nuget:Newtonsoft.Json@13.0.3"),
    "json",
    { fetchImpl },
  );
  assert.equal(formatPackageId(resolution.descriptor.id), "nuget:Newtonsoft.Json@13.0.3");
  assert.equal(resolution.descriptor.integrity, "sha512-base64hash");
  assert.deepEqual(resolution.descriptor.dependencies, {
    Lib: "nuget:Lib@1.0.0",
  });
});

test("OCI adapter resolves a distributable layer", async () => {
  const fetchImpl = async () => ({
    ok: true,
    status: 200,
    json: async () => ({
      schemaVersion: 2,
      layers: [
        {
          mediaType: "application/vnd.oci.image.layer.v1.tar+gzip",
          digest: "sha256:abc123",
        },
      ],
    }),
  });

  const resolution = await resolveOciPackage(
    parsePackageId("oci:ghcr.io/org/image@v1"),
    "image",
    { fetchImpl },
  );
  assert.equal(formatPackageId(resolution.descriptor.id), "oci:ghcr.io/org/image@v1");
  assert.equal(resolution.descriptor.integrity, "sha256:abc123");
  assert.equal(
    resolution.descriptor.resolved,
    "https://ghcr.io/v2/org/image/blobs/sha256:abc123",
  );
});

function jsonResponse(value) {
  return {
    ok: true,
    status: 200,
    json: async () => value,
  };
}

test("store installs and verifies a remote tarball", async (t) => {
  const tarCheck = spawnSync("tar", ["--version"], { encoding: "utf8", windowsHide: true });
  if (tarCheck.status !== 0) {
    t.skip("tar is unavailable");
    return;
  }

  const working = mkdtempSync(join(tmpdir(), "nopm-store-"));
  try {
    const base = join(working, "payload");
    const source = join(base, "package");
    mkdirSync(source, { recursive: true });
    writeFileSync(join(source, "index.txt"), "stored package");
    const archive = join(working, "package.tgz");
    const tar = spawnSync("tar", ["-czf", archive, "-C", base, "package"], {
      encoding: "utf8",
      windowsHide: true,
    });
    assert.equal(tar.status, 0, tar.stderr);
    const bytes = readFileSync(archive);
    const digest = createHash("sha512").update(bytes).digest("base64");
    const resolved = `data:application/gzip;base64,${bytes.toString("base64")}`;

    const project = join(working, "project");
    mkdirSync(project, { recursive: true });
    const lockfile = {
      lockfileVersion: 1,
      generatedBy: "astra/0.1.0",
      manifestHash: "sha256-manifest",
      compiler: { version: "0.1.0", apiVersion: "1.0" },
      semantics: { integers: "wrapping", strings: "utf8" },
      targets: {},
      packages: [
        {
          id: "npm:demo@1.0.0",
          name: "demo",
          source: "npm",
          version: "1.0.0",
          resolved,
          registry: "https://registry.example",
          integrity: `sha512-${digest}`,
          targets: ["web"],
          capabilities: [],
          lifecycleScripts: false,
          dependencies: [],
          commit: null,
        },
      ],
      ffi: {},
    };

    const index = await installLockedPackages(project, lockfile, {
      generatedBy: "nopm/0.1.0",
    });
    assert.equal(index.packages.length, 1);
    const root = index.packages[0].root;
    assert.equal(readFileSync(join(project, root, "index.txt"), "utf8"), "stored package");
  } finally {
    rmSync(working, { recursive: true, force: true });
  }
});




const nopmCliPath = fileURLToPath(new URL("../packages/nopm/dist/cli.js", import.meta.url));

function runNopm(args, cwd) {
  return spawnSync(process.execPath, [nopmCliPath, ...args], {
    cwd,
    encoding: "utf8",
    windowsHide: true,
  });
}

function writePathProject(directory, manifestName = "core.json") {
  const packageDirectory = join(directory, "local-utils");
  mkdirSync(packageDirectory, { recursive: true });

  const packageData = JSON.stringify(
    { package: { name: "local-utils", version: "0.1.0" } },
    null,
    2,
  );
  const buildData = JSON.stringify(
    { targets: { web: { backend: "js", mode: "web" } } },
    null,
    2,
  );
  writeFileSync(join(packageDirectory, "core.json"), `${packageData}\n`);
  writeFileSync(join(packageDirectory, "build.json"), `${buildData}\n`);

  if (manifestName === "core.toml") {
    writeFileSync(
      join(directory, "core.toml"),
      [
        "# demo project",
        "[package]",
        'name = "demo"',
        'version = "0.1.0"',
        "",
      ].join("\n"),
    );
  } else {
    writeFileSync(
      join(directory, "core.json"),
      `${JSON.stringify({ package: { name: "demo", version: "0.1.0" } }, null, 2)}\n`,
    );
  }
  writeFileSync(join(directory, "build.json"), `${buildData}\n`);
  return packageDirectory;
}

test("nopm add and remove update the manifest, lockfile, and store (path source)", () => {
  const directory = mkdtempSync(join(tmpdir(), "nopm-add-"));
  try {
    writePathProject(directory);

    const add = runNopm(["add", "path:local-utils", directory, "--json"], directory);
    assert.equal(add.status, 0, add.stderr);
    assert.equal(JSON.parse(add.stdout).action, "add");

    const manifest = JSON.parse(readFileSync(join(directory, "core.json"), "utf8"));
    assert.deepEqual(manifest.dependencies, { "local-utils": "path:local-utils" });

    const lockfile = readLockfileFile(join(directory, "core.lock.json"));
    assert.deepEqual(lockfile.packages.map((entry) => entry.id), ["path:local-utils"]);

    const store = JSON.parse(
      readFileSync(join(directory, ".astra", "store", "index.json"), "utf8"),
    );
    assert.deepEqual(store.packages.map((entry) => entry.id), ["path:local-utils"]);

    const duplicate = runNopm(["add", "path:local-utils", directory], directory);
    assert.notEqual(duplicate.status, 0);
    assert.match(duplicate.stderr, /NP0013/);

    const remove = runNopm(["remove", "local-utils", directory], directory);
    assert.equal(remove.status, 0, remove.stderr);
    const removed = JSON.parse(readFileSync(join(directory, "core.json"), "utf8"));
    assert.equal(Object.hasOwn(removed, "dependencies"), false);
    const relocked = readLockfileFile(join(directory, "core.lock.json"));
    assert.deepEqual(relocked.packages, []);

    const missing = runNopm(["remove", "local-utils", directory], directory);
    assert.notEqual(missing.status, 0);
    assert.match(missing.stderr, /NP0014/);
  } finally {
    rmSync(directory, { recursive: true, force: true });
  }
});

test("nopm add and remove preserve TOML comments through the CLI", () => {
  const directory = mkdtempSync(join(tmpdir(), "nopm-add-toml-"));
  try {
    writePathProject(directory, "core.toml");

    const add = runNopm(["add", "path:local-utils", directory], directory);
    assert.equal(add.status, 0, add.stderr);
    let source = readFileSync(join(directory, "core.toml"), "utf8");
    assert.match(source, /^# demo project$/m);
    assert.match(source, /^\[dependencies\]$/m);
    assert.match(source, /^local-utils = "path:local-utils"$/m);

    const remove = runNopm(["remove", "local-utils", directory], directory);
    assert.equal(remove.status, 0, remove.stderr);
    source = readFileSync(join(directory, "core.toml"), "utf8");
    assert.match(source, /^# demo project$/m);
    assert.match(source, /^name = "demo"$/m);
    assert.doesNotMatch(source, /^\[dependencies\]$/m);
  } finally {
    rmSync(directory, { recursive: true, force: true });
  }
});

test("nopm add --dry-run writes neither the manifest nor the lockfile", () => {
  const directory = mkdtempSync(join(tmpdir(), "nopm-add-dry-"));
  try {
    writePathProject(directory);
    const manifestPath = join(directory, "core.json");
    const before = readFileSync(manifestPath, "utf8");

    const result = runNopm(["add", "path:local-utils", directory, "--dry-run"], directory);
    assert.equal(result.status, 0, result.stderr);
    assert.match(result.stdout, /Would add path:local-utils/);
    assert.equal(readFileSync(manifestPath, "utf8"), before);
    assert.equal(existsSync(join(directory, "core.lock.json")), false);
  } finally {
    rmSync(directory, { recursive: true, force: true });
  }
});

test("nopm add restores the manifest and lockfile when resolution fails", () => {
  const directory = mkdtempSync(join(tmpdir(), "nopm-add-rollback-"));
  try {
    writePathProject(directory);
    const locked = runNopm(["lock", directory], directory);
    assert.equal(locked.status, 0, locked.stderr);

    const manifestPath = join(directory, "core.json");
    const lockfilePath = join(directory, "core.lock.json");
    const manifestBefore = readFileSync(manifestPath, "utf8");
    const lockfileBefore = readFileSync(lockfilePath, "utf8");

    const failed = runNopm(["add", "path:missing-package", directory], directory);
    assert.notEqual(failed.status, 0);

    assert.equal(readFileSync(manifestPath, "utf8"), manifestBefore);
    assert.equal(readFileSync(lockfilePath, "utf8"), lockfileBefore);
  } finally {
    rmSync(directory, { recursive: true, force: true });
  }
});

test("sys planner builds the exact command lines for every manager", () => {
  const cases = [
    [
      "win32",
      0,
      ["winget"],
      "git",
      "winget",
      [["winget", "install", "--id", "Git.Git", "-e", "--accept-package-agreements", "--accept-source-agreements", "--silent"]],
    ],
    ["darwin", 501, ["brew"], "cmake", "brew", [["brew", "install", "cmake"]]],
    [
      "linux",
      0,
      ["apt-get"],
      "clang",
      "apt-get",
      [["apt-get", "update"], ["apt-get", "install", "-y", "clang"]],
    ],
    [
      "linux",
      1000,
      ["apt-get"],
      "clang",
      "apt-get",
      [["sudo", "-n", "apt-get", "update"], ["sudo", "-n", "apt-get", "install", "-y", "clang"]],
    ],
    ["linux", 0, ["dnf"], "git", "dnf", [["dnf", "install", "-y", "git"]]],
    ["linux", 0, ["pacman"], "git", "pacman", [["pacman", "-S", "--noconfirm", "git"]]],
    ["linux", 0, ["zypper"], "git", "zypper", [["zypper", "--non-interactive", "install", "git"]]],
    ["linux", 0, ["apk"], "git", "apk", [["apk", "add", "git"]]],
  ];

  for (const [platform, uid, managers, tool, manager, commands] of cases) {
    const plan = planSysInstall(tool, { platform, uid, managers });
    assert.equal(plan.manager, manager, `${platform} ${managers.join(",")}`);
    assert.deepEqual(plan.commands, commands, `${platform} ${managers.join(",")}`);
    assert.equal(plan.installed, false);
  }
});

test("sys planner rejects unknown tools and missing managers", () => {
  expectCode(
    () => planSysInstall("bogus", { platform: "linux", uid: 0, managers: ["apt-get"] }),
    "NP0015",
  );
  expectCode(() => planSysInstall("git", { platform: "linux", uid: 0, managers: [] }), "NP0017");
  expectCode(() => planSysInstall("git", { platform: "win32", uid: 0, managers: [] }), "NP0017");
  expectCode(
    () => planSysInstall("git", { platform: "darwin", uid: 0, managers: ["brew"] }),
    "NP0017",
  );
});

test("sys manager detection asks the injected presence check", () => {
  assert.deepEqual(detectManagers("win32", () => false), []);
  assert.deepEqual(detectManagers("win32", (command) => command === "winget"), ["winget"]);
  assert.deepEqual(detectManagers("darwin", (command) => command === "brew"), ["brew"]);
  assert.deepEqual(
    detectManagers("linux", (_command, args) => args[1] === "command -v dnf"),
    ["dnf"],
  );
});

test("sys executor runs the plan in order and short-circuits when installed", () => {
  const context = { platform: "linux", uid: 1000, managers: ["apt-get"] };
  const calls = [];

  const runner = (installed) => ({
    run(command, args) {
      calls.push([command, ...args]);
      if (command === "git" && args[0] === "--version") {
        return { status: installed ? 0 : 1, stdout: "", stderr: "" };
      }
      return { status: 0, stdout: "", stderr: "" };
    },
  });

  const plan = executeSysInstall("git", context, runner(false));
  assert.deepEqual(plan.commands, [
    ["sudo", "-n", "apt-get", "update"],
    ["sudo", "-n", "apt-get", "install", "-y", "git"],
  ]);
  assert.deepEqual(calls, [
    ["git", "--version"],
    ["sudo", "-n", "apt-get", "update"],
    ["sudo", "-n", "apt-get", "install", "-y", "git"],
  ]);

  calls.length = 0;
  const already = executeSysInstall("git", context, runner(true));
  assert.equal(already.installed, true);
  assert.deepEqual(already.commands, []);
  assert.deepEqual(calls, [["git", "--version"]]);
});

test("sys executor reports a failed install as NP0018", () => {
  const runner = {
    run(command, args) {
      if (command === "git" && args[0] === "--version") {
        return { status: 1, stdout: "", stderr: "" };
      }
      return { status: 3, stdout: "", stderr: "boom" };
    },
  };

  assert.throws(
    () => executeSysInstall("git", { platform: "linux", uid: 0, managers: ["apt-get"] }, runner),
    (error) => {
      assert.ok(error instanceof NopmError);
      assert.equal(error.code, "NP0018");
      assert.match(error.message, /apt-get/);
      return true;
    },
  );
});

test("nopm add sys: writes nothing, and remove sys: is refused", () => {
  const directory = mkdtempSync(join(tmpdir(), "nopm-sys-"));
  try {
    writeFileSync(
      join(directory, "core.json"),
      `${JSON.stringify({ package: { name: "demo", version: "0.1.0" } }, null, 2)}\n`,
    );
    const before = readFileSync(join(directory, "core.json"), "utf8");

    const dry = runNopm(["add", "sys:git", "--dry-run", "--json"], directory);
    assert.equal(dry.status, 0, dry.stderr);
    const payload = JSON.parse(dry.stdout);
    assert.equal(payload.action, "sys-install");
    assert.equal(payload.tool, "git");
    assert.equal(typeof payload.installed, "boolean");
    assert.ok(Array.isArray(payload.commands));
    assert.equal(readFileSync(join(directory, "core.json"), "utf8"), before);
    assert.equal(existsSync(join(directory, "core.lock.json")), false);
    assert.equal(existsSync(join(directory, ".astra")), false);

    const remove = runNopm(["remove", "sys:git"], directory);
    assert.notEqual(remove.status, 0);
    assert.match(remove.stderr, /NP0016/);

    const extra = runNopm(["add", "sys:git", directory], directory);
    assert.notEqual(extra.status, 0);
    assert.match(extra.stderr, /at most 1/);

    const unknown = runNopm(["add", "sys:bogus-not-real", "--dry-run"], directory);
    assert.notEqual(unknown.status, 0);
    assert.match(unknown.stderr, /NP0015/);

    // A real command that is not a supported system tool must still fail the
    // name check; being on PATH is not an installation plan.
    const installedButUnknown = runNopm(["add", "sys:node", "--dry-run"], directory);
    assert.notEqual(installedButUnknown.status, 0);
    assert.match(installedButUnknown.stderr, /NP0015/);
  } finally {
    rmSync(directory, { recursive: true, force: true });
  }
});
