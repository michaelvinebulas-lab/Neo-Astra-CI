import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import {
  existsSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";

const root = fileURLToPath(new URL("../..", import.meta.url));
const version = JSON.parse(
  readFileSync(join(root, "package.json"), "utf8"),
).version;
const executable = process.platform === "win32" ? ".exe" : "";
const osName = { win32: "windows", linux: "linux", darwin: "darwin" }[
  process.platform
];
const target = osName === undefined ? undefined : `neo-astra-${osName}-${process.arch}`;
const supported = new Set([
  "neo-astra-windows-x64",
  "neo-astra-linux-x64",
  "neo-astra-darwin-arm64",
]).has(target);

function run(args, expectedStatus) {
  const result = spawnSync(process.execPath, args, {
    cwd: root,
    encoding: "utf8",
  });
  assert.equal(
    result.status,
    expectedStatus,
    `${args.join(" ")} stdout:\n${result.stdout}\nstderr:\n${result.stderr}`,
  );
  return result;
}

test("npm meta package exposes the native commands and platform packages", () => {
  const out = mkdtempSync(join(tmpdir(), "astra-npm-meta-"));
  try {
    run(["scripts/package-npm.mjs", "--meta", "--out", out], 0);
    const meta = join(out, "meta");
    const manifest = JSON.parse(
      readFileSync(join(meta, "package.json"), "utf8"),
    );
    assert.equal(manifest.name, "neo-astra");
    assert.equal(manifest.version, version);
    assert.deepEqual(manifest.bin, {
      astra: "./bin/astra.exe",
      nopm: "./bin/nopm.exe",
      nova: "./bin/nova.exe",
      "neo-astra": "./bin/astra.exe",
    });
    assert.deepEqual(manifest.optionalDependencies, {
      "neo-astra-windows-x64": version,
      "neo-astra-linux-x64": version,
      "neo-astra-darwin-arm64": version,
    });
    assert.equal(manifest.scripts.postinstall, "node ./postinstall.mjs");
    for (const file of [
      "postinstall.mjs",
      "README.md",
      "bin/astra.exe",
      "bin/nopm.exe",
      "bin/nova.exe",
    ]) {
      assert.ok(existsSync(join(meta, file)), `${file} is missing`);
    }
  } finally {
    rmSync(out, { recursive: true, force: true });
  }
});

test(
  "npm platform package contains the native toolchain",
  { skip: !supported },
  () => {
    const out = mkdtempSync(join(tmpdir(), "astra-npm-platform-"));
    try {
      const source = join(out, "source");
      const files = [
        `bin/astra${executable}`,
        `bin/astra-compiler${executable}`,
        `bin/nova${executable}`,
        `bin/nopm${executable}`,
        `typescript/tsc${executable}`,
        "typescript/package.json",
      ];
      for (const file of files) {
        const path = join(source, file);
        mkdirSync(dirname(path), { recursive: true });
        writeFileSync(path, "fixture\n", "utf8");
      }

      run(
        [
          "scripts/package-npm.mjs",
          "--platform",
          "--source",
          source,
          "--out",
          out,
        ],
        0,
      );
      const platform = join(out, "platform");
      const manifest = JSON.parse(
        readFileSync(join(platform, "package.json"), "utf8"),
      );
      assert.equal(manifest.name, target);
      assert.equal(manifest.version, version);
      assert.deepEqual(manifest.os, [process.platform]);
      assert.deepEqual(manifest.cpu, [process.arch]);
      assert.ok(manifest.files.includes("bin"));
      assert.ok(manifest.files.includes("typescript"));
      for (const file of files) {
        assert.ok(existsSync(join(platform, file)), `${file} is missing`);
      }
    } finally {
      rmSync(out, { recursive: true, force: true });
    }
  },
);
