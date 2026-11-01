import test from "node:test";
import assert from "node:assert/strict";
import { mkdtemp, rm, writeFile } from "node:fs/promises";
import { mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

import {
  ManifestError,
  findManifest,
  findBuildManifest,
  loadBuildManifest,
  findManifests,
  loadProjectManifests,
  applyBuildDefaults,
  validateManifestKeys,
  formatManifestSource,
  getPackageName,
  loadManifest,
  manifestFileForPath,
} from "../packages/manifest/dist/index.js";
import { runCli } from "../packages/cli/dist/index.js";

async function createProject() {
  return mkdtemp(join(tmpdir(), "neo-astra-"));
}

async function removeProject(directory) {
  await rm(directory, { recursive: true, force: true });
}

test("loads core.json and reads the package name", async () => {
  const directory = await createProject();

  try {
    await writeFile(
      join(directory, "core.json"),
      JSON.stringify({ package: { name: "demo" } }),
      "utf8",
    );

    const manifest = loadManifest(directory);
    assert.equal(manifest.file.format, "json");
    assert.equal(manifest.file.name, "core.json");
    assert.equal(getPackageName(manifest), "demo");
  } finally {
    await removeProject(directory);
  }
});

test("loads core.toml and reads the package name", async () => {
  const directory = await createProject();

  try {
    await writeFile(join(directory, "core.toml"), '[package]\nname = "demo"\n', "utf8");

    const manifest = loadManifest(directory);
    assert.equal(manifest.file.format, "toml");
    assert.equal(manifest.file.name, "core.toml");
    assert.equal(getPackageName(manifest), "demo");
  } finally {
    await removeProject(directory);
  }
});

test("rejects a project with both manifest files", async () => {
  const directory = await createProject();

  try {
    await writeFile(join(directory, "core.json"), "{}", "utf8");
    await writeFile(join(directory, "core.toml"), "", "utf8");

    assert.throws(
      () => findManifest(directory),
      (error) => error instanceof ManifestError && error.code === "NS9001",
    );
  } finally {
    await removeProject(directory);
  }
});

test("formats a JSON manifest in canonical key order", async () => {
  const directory = await createProject();

  try {
    const path = join(directory, "build.json");
    const file = manifestFileForPath(path);
    assert.ok(file);

    const source = JSON.stringify({
      targets: { web: { mode: "web", backend: "js" } },
      build: { defaultTarget: "web", defaultMode: "app" },
    });
    const formatted = await formatManifestSource(source, file);

    assert.equal(
      formatted,
      `{
  "build": {
    "defaultTarget": "web",
    "defaultMode": "app"
  },
  "targets": {
    "web": {
      "backend": "js",
      "mode": "web"
    }
  }
}
`,
    );
    assert.equal(await formatManifestSource(formatted, file), formatted);
  } finally {
    await removeProject(directory);
  }
});

test("orders package sources canonically", async () => {
  const directory = await createProject();

  try {
    const file = manifestFileForPath(join(directory, "core.json"));
    assert.ok(file);

    const source = JSON.stringify({
      packageSources: {
        npm: { enabled: true },
        path: { enabled: true },
        cargo: { enabled: true },
        git: { enabled: true },
      },
    });
    const formatted = await formatManifestSource(source, file);
    const packageSources = Object.keys(JSON.parse(formatted).packageSources);

    assert.deepEqual(packageSources, ["path", "git", "npm", "cargo"]);
  } finally {
    await removeProject(directory);
  }
});

test("formats a TOML manifest and preserves comments", async () => {
  const directory = await createProject();

  try {
    const path = join(directory, "core.toml");
    const file = manifestFileForPath(path);
    assert.ok(file);

    const source = '# top\n[package]\nname="demo" # inline\nversion = "1.0.0"\n';
    const formatted = await formatManifestSource(source, file);

    assert.match(formatted, /# top/);
    assert.match(formatted, /name = "demo"\s+# inline/);
    assert.equal(await formatManifestSource(formatted, file), formatted);
  } finally {
    await removeProject(directory);
  }
});

test("rejects duplicate JSON keys during formatting", async () => {
  const directory = await createProject();

  try {
    const file = manifestFileForPath(join(directory, "core.json"));
    assert.ok(file);

    await assert.rejects(
      () =>
        formatManifestSource(
          '{"package":{"name":"one","name":"two"}}',
          file,
        ),
      (error) =>
        error instanceof ManifestError &&
        error.code === "NS9002" &&
        /Duplicate key 'name'/.test(error.message),
    );
  } finally {
    await removeProject(directory);
  }
});

test("rejects invalid manifest source during formatting", async () => {
  const directory = await createProject();

  try {
    const file = manifestFileForPath(join(directory, "core.json"));
    assert.ok(file);

    await assert.rejects(
      () => formatManifestSource("{ \"package\":", file),
      (error) => error instanceof ManifestError && error.code === "NS9002",
    );
  } finally {
    await removeProject(directory);
  }
});

test("CLI check loads a manifest", async () => {
  const directory = await createProject();

  try {
    await writeFile(
      join(directory, "core.json"),
      JSON.stringify({ package: { name: "demo" } }),
      "utf8",
    );

    const originalWrite = process.stdout.write;
    let output = "";
    process.stdout.write = (chunk) => {
      output += String(chunk);
      return true;
    };

    try {
      assert.equal(await runCli(["check", directory, "--json"], directory), 0);
      assert.match(output, /"package": "demo"/);
    } finally {
      process.stdout.write = originalWrite;
    }
  } finally {
    await removeProject(directory);
  }
});
test("rejects a build key in core.json", () => {
  const directory = mkdtempSync(join(tmpdir(), "neo-astra-"));
  try {
    writeFileSync(
      join(directory, "core.json"),
      JSON.stringify({ package: { name: "demo" }, build: { defaultTarget: "web" } }),
      "utf8",
    );
    assert.throws(
      () => loadManifest(directory),
      (error) => error instanceof ManifestError && error.code === "NS9002" && /build\.json/.test(error.message),
    );
  } finally {
    rmSync(directory, { recursive: true, force: true });
  }
});

test("rejects a core key in build.json", () => {
  const directory = mkdtempSync(join(tmpdir(), "neo-astra-"));
  try {
    writeFileSync(join(directory, "core.json"), JSON.stringify({ package: { name: "demo" } }), "utf8");
    writeFileSync(join(directory, "build.json"), JSON.stringify({ package: { name: "x" } }), "utf8");
    assert.throws(
      () => loadBuildManifest(directory),
      (error) => error instanceof ManifestError && error.code === "NS9002" && /core\.json/.test(error.message),
    );
  } finally {
    rmSync(directory, { recursive: true, force: true });
  }
});

test("formats a core.json in canonical key order", async () => {
  const directory = mkdtempSync(join(tmpdir(), "neo-astra-"));
  try {
    const path = join(directory, "core.json");
    const file = manifestFileForPath(path);
    assert.ok(file);
    const source = JSON.stringify({
      ffi: { "lib": { source: "native", type: "c", ownership: "borrowed" } },
      package: { version: "1.0.0", name: "demo" },
      dependencies: { z: "nova:z@1", a: "nova:a@1" },
    });
    const formatted = await formatManifestSource(source, file);
    assert.equal(
      formatted,
      `{
  "package": {
    "name": "demo",
    "version": "1.0.0"
  },
  "dependencies": {
    "a": "nova:a@1",
    "z": "nova:z@1"
  },
  "ffi": {
    "lib": {
      "source": "native",
      "type": "c",
      "ownership": "borrowed"
    }
  }
}
`,
    );
    assert.equal(await formatManifestSource(formatted, file), formatted);
  } finally {
    rmSync(directory, { recursive: true, force: true });
  }
});

test("CLI check rejects a build key in core.json", async () => {
  const directory = await mkdtemp(join(tmpdir(), "neo-astra-"));
  try {
    await writeFile(
      join(directory, "core.json"),
      JSON.stringify({ package: { name: "demo" }, build: { defaultTarget: "web" } }),
      "utf8",
    );
    const originalWrite = process.stdout.write;
    const originalError = process.stderr.write;
    let stderr = "";
    process.stdout.write = () => true;
    process.stderr.write = (chunk) => {
      stderr += String(chunk);
      return true;
    };
    let code;
    try {
      code = await runCli(["check", directory], directory);
    } finally {
      process.stdout.write = originalWrite;
      process.stderr.write = originalError;
    }
    assert.notEqual(code, 0);
    assert.match(stderr, /NS9002/);
    assert.match(stderr, /build\.json/);
  } finally {
    await rm(directory, { recursive: true, force: true });
  }
});
test("applies build defaults when build.* is absent", () => {
  const directory = mkdtempSync(join(tmpdir(), "neo-astra-"));
  try {
    writeFileSync(join(directory, "core.json"), JSON.stringify({ package: { name: "demo" } }), "utf8");
    const manifests = loadProjectManifests(directory);
    assert.equal(manifests.build, undefined);
    const data = applyBuildDefaults(manifests.build?.data);
    assert.equal(data.build.defaultTarget, "native");
    assert.equal(data.nova.entry, "src/main.ns");
    assert.equal(data.math.integers, "wrapping");
    assert.equal(data.toolchain.c, "auto");
  } finally {
    rmSync(directory, { recursive: true, force: true });
  }
});

test("formats a build.toml and preserves comments", async () => {
  const directory = mkdtempSync(join(tmpdir(), "neo-astra-"));
  try {
    const path = join(directory, "build.toml");
    const file = manifestFileForPath(path);
    assert.ok(file);
    const source = '# top\n[build]\ndefaultTarget = "web" # inline\n';
    const formatted = await formatManifestSource(source, file);
    assert.match(formatted, /# top/);
    assert.match(formatted, /defaultTarget = "web"\s+# inline/);
    assert.equal(await formatManifestSource(formatted, file), formatted);
  } finally {
    rmSync(directory, { recursive: true, force: true });
  }
});
