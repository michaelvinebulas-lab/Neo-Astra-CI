import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import {
  copyFileSync,
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
        "lib/cli/dist/index.js",
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
      assert.ok(manifest.files.includes("lib"));
      for (const file of files) {
        assert.ok(existsSync(join(platform, file)), `${file} is missing`);
      }
    } finally {
      rmSync(out, { recursive: true, force: true });
    }
  },
);

test(
  "npm postinstall copies the native toolchain and bridge payload",
  { skip: !supported },
  () => {
    const out = mkdtempSync(join(tmpdir(), "astra-npm-postinstall-"));
    try {
      const meta = join(out, "meta");
      mkdirSync(meta, { recursive: true });
      writeFileSync(
        join(meta, "package.json"),
        `${JSON.stringify(
          {
            name: "neo-astra",
            version,
            optionalDependencies: { [target]: version },
          },
          null,
          2,
        )}\n`,
        "utf8",
      );
      copyFileSync(
        join(root, "scripts", "npm", "postinstall.mjs"),
        join(meta, "postinstall.mjs"),
      );

      const platformPackage = join(meta, "node_modules", target);
      const bin = join(platformPackage, "bin");
      const typescript = join(platformPackage, "typescript");
      const bridge = join(platformPackage, "lib", "cli", "dist");
      mkdirSync(bin, { recursive: true });
      mkdirSync(typescript, { recursive: true });
      mkdirSync(bridge, { recursive: true });
      writeFileSync(
        join(platformPackage, "package.json"),
        `${JSON.stringify({ name: target, version }, null, 2)}\n`,
        "utf8",
      );

      // Copy the running Node binary as the two binaries postinstall executes.
      // `node --version` exits zero, so verify() exercises the copied paths
      // without needing a native build in the test fixture.
      copyFileSync(process.execPath, join(bin, `astra${executable}`));
      copyFileSync(process.execPath, join(bin, `nopm${executable}`));
      for (const file of [
        `astra-compiler${executable}`,
        `nova${executable}`,
      ]) {
        writeFileSync(join(bin, file), "fixture\n", "utf8");
      }
      writeFileSync(
        join(typescript, `tsc${executable}`),
        "fixture\n",
        "utf8",
      );
      writeFileSync(join(typescript, "lib.d.ts"), "fixture\n", "utf8");
      writeFileSync(join(bridge, "index.js"), "fixture\n", "utf8");

      const result = spawnSync(
        process.execPath,
        [join(meta, "postinstall.mjs")],
        {
          cwd: meta,
          encoding: "utf8",
          windowsHide: true,
        },
      );
      assert.equal(
        result.status,
        0,
        `postinstall stdout:\n${result.stdout}\nstderr:\n${result.stderr}`,
      );
      for (const file of [
        "bin/astra.exe",
        "bin/nopm.exe",
        "bin/nova.exe",
        `bin/astra-compiler${executable}`,
        `typescript/tsc${executable}`,
        "typescript/lib.d.ts",
        "lib/cli/dist/index.js",
      ]) {
        assert.ok(existsSync(join(meta, file)), `${file} is missing`);
      }
    } finally {
      rmSync(out, { recursive: true, force: true });
    }
  },
);
