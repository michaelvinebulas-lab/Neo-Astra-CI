import test from "node:test";
import assert from "node:assert/strict";
import { execFileSync, spawnSync } from "node:child_process";
import {
  mkdir,
  mkdtemp,
  readFile,
  readdir,
  rm,
  writeFile,
} from "node:fs/promises";
import { existsSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join, relative } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..", "..");
const suffix = process.platform === "win32" ? ".exe" : "";
const enabled = process.env.ASTRA_NATIVE_PARITY === "1";
const skip = enabled ? false : "set ASTRA_NATIVE_PARITY=1 to run the native parity gate";

const tsCli = join(root, "orchestrator", "packages", "cli", "dist", "index.js");
const tsNopm = join(root, "orchestrator", "packages", "nopm", "dist", "cli.js");

function findBinary(names) {
  for (const name of names) {
    const candidate = join(root, "target", "debug", `${name}${suffix}`);
    if (existsSync(candidate)) {
      return candidate;
    }
  }
  throw new Error(
    `native binary not found: ${names.join(", ")} under target/debug; run cargo build first`,
  );
}

function nativeCli() {
  return findBinary(["astra", "astra-native"]);
}

function nativeNopm() {
  return findBinary(["nopm"]);
}

if (enabled) {
  execFileSync(
    "cargo",
    ["build", "-q", "-p", "astra-cli", "-p", "astra-nopm"],
    { cwd: root, stdio: "inherit" },
  );
}

function runNode(args, cwd) {
  return spawnSync(process.execPath, [tsCli, ...args], {
    cwd,
    encoding: "utf8",
    windowsHide: true,
  });
}

function runNative(args, cwd) {
  return spawnSync(nativeCli(), args, {
    cwd,
    encoding: "utf8",
    windowsHide: true,
  });
}

function runNodeNopm(args, cwd) {
  return spawnSync(process.execPath, [tsNopm, ...args], {
    cwd,
    encoding: "utf8",
    windowsHide: true,
  });
}

function runNativeNopm(args, cwd) {
  return spawnSync(nativeNopm(), args, {
    cwd,
    encoding: "utf8",
    windowsHide: true,
  });
}

function normalize(value, directory) {
  if (value === undefined || value === null) {
    return value;
  }
  return value
    .split(directory.replaceAll("\\", "\\\\")).join("<project>")
    .split(directory).join("<project>")
    .split(directory.replaceAll("\\", "/")).join("<project>");
}

function compareResult(label, expected, actual, expectedDir, actualDir) {
  assert.equal(
    actual.status,
    expected.status,
    `${label}: status\nexpected stdout:\n${expected.stdout}\nexpected stderr:\n${expected.stderr}\nactual stdout:\n${actual.stdout}\nactual stderr:\n${actual.stderr}`,
  );
  assert.equal(
    normalize(actual.stdout, actualDir),
    normalize(expected.stdout, expectedDir),
    `${label}: stdout\nactual stderr:\n${actual.stderr}`,
  );
  assert.equal(
    normalize(actual.stderr, actualDir),
    normalize(expected.stderr, expectedDir),
    `${label}: stderr`,
  );
}

async function makeProject(source, options = {}) {
  const directory = await mkdtemp(join(tmpdir(), "astra-native-parity-"));
  const packageName = options.packageName ?? "parity";
  await mkdir(join(directory, "src"), { recursive: true });
  await writeFile(
    join(directory, "core.json"),
    `${JSON.stringify(
      { package: { name: packageName, version: "0.1.0" } },
      null,
      2,
    )}\n`,
    "utf8",
  );
  const build = options.build ?? {
    build: { defaultTarget: "native" },
    nova: { entry: "src/main.ns" },
    targets: {
      native: { backend: "native", mode: "core", outDir: "dist/native" },
      web: { backend: "js", mode: "core", outDir: "dist/web" },
      ts: { backend: "ts", mode: "core", outDir: "dist/ts" },
    },
    math: { integers: "wrapping" },
    toolchain: { c: "auto" },
  };
  await writeFile(
    join(directory, "build.json"),
    `${JSON.stringify(build, null, 2)}\n`,
    "utf8",
  );
  await writeFile(join(directory, "src", "main.ns"), source, "utf8");
  return directory;
}

async function pair(projectOptions, run) {
  const expectedDir = await makeProject(projectOptions.source, projectOptions);
  const actualDir = await makeProject(projectOptions.source, projectOptions);
  const expected = run(expectedDir, false);
  const actual = run(actualDir, true);
  compareResult("parity", expected, actual, expectedDir, actualDir);
  return { expectedDir, actualDir, expected, actual };
}

async function listFiles(directory, base = directory) {
  const entries = await readdir(directory, { withFileTypes: true });
  const files = [];
  for (const entry of entries) {
    const path = join(directory, entry.name);
    if (entry.isDirectory()) {
      files.push(...(await listFiles(path, base)));
    } else {
      files.push(relative(base, path).replaceAll("\\", "/"));
    }
  }
  return files.sort();
}

async function compareTree(label, expectedDir, actualDir) {
  const expectedFiles = await listFiles(expectedDir);
  const actualFiles = await listFiles(actualDir);
  assert.deepEqual(actualFiles, expectedFiles, `${label}: file list`);
  for (const file of expectedFiles) {
    const expected = await readFile(join(expectedDir, file), "utf8");
    const actual = await readFile(join(actualDir, file), "utf8");
    assert.equal(actual, expected, `${label}: ${file}`);
  }
}

const VALID = `module app.main

pub fn main() -> i32 {
    return 0
}
`;

const BROKEN = `module app.main

pub fn main() -> i32 {
    return "no"
}
`;

test("native help and version match the shipped CLI", { skip }, async () => {
  const directory = await mkdtemp(join(tmpdir(), "astra-native-help-"));
  try {
    for (const args of [["--help"], ["--version"], ["help"], ["version"]]) {
      compareResult(
        `astra ${args.join(" ")}`,
        runNode(args, directory),
        runNative(args, directory),
        directory,
        directory,
      );
    }
  } finally {
    await rm(directory, { recursive: true, force: true });
  }
});

test("native check matches valid, broken, and JSON output", { skip }, async () => {
  for (const [label, source] of [
    ["valid", VALID],
    ["broken", BROKEN],
  ]) {
    for (const extra of [[], ["--json"]]) {
      const expectedDir = await makeProject(source);
      const actualDir = await makeProject(source);
      try {
        compareResult(
          `check ${label} ${extra.join(" ")}`,
          runNode(["check", expectedDir, ...extra], expectedDir),
          runNative(["check", actualDir, ...extra], actualDir),
          expectedDir,
          actualDir,
        );
      } finally {
        await rm(expectedDir, { recursive: true, force: true });
        await rm(actualDir, { recursive: true, force: true });
      }
    }
  }
});

test("native fmt matches check mode, rewriting, and manifests", { skip }, async () => {
  const expectedDir = await makeProject("pub fn main()->i32{return 0}\n");
  const actualDir = await makeProject("pub fn main()->i32{return 0}\n");
  await writeFile(
    join(expectedDir, "core.json"),
    '{"package":{"name":"parity"}}\n',
    "utf8",
  );
  await writeFile(
    join(actualDir, "core.json"),
    '{"package":{"name":"parity"}}\n',
    "utf8",
  );
  try {
    for (const args of [
      ["fmt", "--check", "--json"],
      ["fmt", "--json"],
    ]) {
      compareResult(
        `fmt ${args.join(" ")}`,
        runNode([...args, expectedDir], expectedDir),
        runNative([...args, actualDir], actualDir),
        expectedDir,
        actualDir,
      );
    }
    compareResult(
      "fmt already formatted",
      runNode(["fmt", expectedDir, "--check", "--json"], expectedDir),
      runNative(["fmt", actualDir, "--check", "--json"], actualDir),
      expectedDir,
      actualDir,
    );
    await compareTree("fmt", expectedDir, actualDir);
  } finally {
    await rm(expectedDir, { recursive: true, force: true });
    await rm(actualDir, { recursive: true, force: true });
  }
});

test("native ast and explain match for a project entry", { skip }, async () => {
  const expectedDir = await makeProject(VALID);
  const actualDir = await makeProject(VALID);
  try {
    for (const command of ["ast", "explain"]) {
      for (const extra of [[], ["--json"]]) {
        compareResult(
          `${command} ${extra.join(" ")}`,
          runNode([command, expectedDir, ...extra], expectedDir),
          runNative([command, actualDir, ...extra], actualDir),
          expectedDir,
          actualDir,
        );
      }
    }
    compareResult(
      "ast with explicit mode and target",
      runNode(["ast", expectedDir, "--mode", "core", "--target", "js"], expectedDir),
      runNative(["ast", actualDir, "--mode", "core", "--target", "js"], actualDir),
      expectedDir,
      actualDir,
    );
  } finally {
    await rm(expectedDir, { recursive: true, force: true });
    await rm(actualDir, { recursive: true, force: true });
  }
});

test("native C and native executable builds match generated artifacts", { skip }, async () => {
  for (const target of ["c", "native"]) {
    const expectedDir = await makeProject(VALID);
    const actualDir = await makeProject(VALID);
    try {
      compareResult(
        `build --target ${target} --json`,
        runNode(["build", expectedDir, "--target", target, "--json"], expectedDir),
        runNative(["build", actualDir, "--target", target, "--json"], actualDir),
        expectedDir,
        actualDir,
      );

      const expectedFiles = await listFiles(expectedDir);
      const actualFiles = await listFiles(actualDir);
      assert.deepEqual(actualFiles, expectedFiles, `build ${target}: file list`);
      for (const file of expectedFiles) {
        if (file.endsWith(".exe") || file === "main") {
          continue;
        }
        assert.equal(
          await readFile(join(actualDir, file), "utf8"),
          await readFile(join(expectedDir, file), "utf8"),
          `build ${target}: ${file}`,
        );
      }

      if (target === "native") {
        const exeName = process.platform === "win32" ? "main.exe" : "main";
        const expectedRun = spawnSync(
          join(expectedDir, "dist", "native", exeName),
          [],
          { encoding: "utf8", windowsHide: true },
        );
        const actualRun = spawnSync(
          join(actualDir, "dist", "native", exeName),
          [],
          { encoding: "utf8", windowsHide: true },
        );
        assert.equal(actualRun.status, expectedRun.status, "native executable status");
        assert.equal(actualRun.stdout, expectedRun.stdout, "native executable stdout");
        assert.equal(actualRun.stderr, expectedRun.stderr, "native executable stderr");
      }
    } finally {
      await rm(expectedDir, { recursive: true, force: true });
      await rm(actualDir, { recursive: true, force: true });
    }
  }
});

test("native JS build matches generated module tree", { skip }, async () => {
  const expectedDir = await makeProject(VALID);
  const actualDir = await makeProject(VALID);
  try {
    compareResult(
      "build --target web --json",
      runNode(["build", expectedDir, "--target", "web", "--json"], expectedDir),
      runNative(["build", actualDir, "--target", "web", "--json"], actualDir),
      expectedDir,
      actualDir,
    );
    await compareTree("build", expectedDir, actualDir);
  } finally {
    await rm(expectedDir, { recursive: true, force: true });
    await rm(actualDir, { recursive: true, force: true });
  }
});

test("native doctor matches the JSON ladder", { skip }, async () => {
  const directory = await mkdtemp(join(tmpdir(), "astra-native-doctor-"));
  try {
    compareResult(
      "doctor --json",
      runNode(["doctor", directory, "--json"], directory),
      runNative(["doctor", directory, "--json"], directory),
      directory,
      directory,
    );
  } finally {
    await rm(directory, { recursive: true, force: true });
  }
});

async function makeNopmProject() {
  const directory = await mkdtemp(join(tmpdir(), "astra-native-nopm-"));
  const packageDirectory = join(directory, "local-utils");
  await mkdir(join(packageDirectory, "src"), { recursive: true });
  await writeFile(
    join(packageDirectory, "core.json"),
    `${JSON.stringify({ package: { name: "local-utils", version: "0.1.0" } }, null, 2)}\n`,
    "utf8",
  );
  await writeFile(
    join(packageDirectory, "build.json"),
    `${JSON.stringify({ targets: { web: { backend: "js", mode: "web" } } }, null, 2)}\n`,
    "utf8",
  );
  await writeFile(
    join(packageDirectory, "src", "lib.ns"),
    "module local_utils.lib\n\npub fn answer() -> i32 { return 42 }\n",
    "utf8",
  );
  await writeFile(
    join(directory, "core.json"),
    `${JSON.stringify({ package: { name: "demo", version: "0.1.0" } }, null, 2)}\n`,
    "utf8",
  );
  await writeFile(
    join(directory, "build.json"),
    `${JSON.stringify({ targets: { web: { backend: "js", mode: "web" } } }, null, 2)}\n`,
    "utf8",
  );
  return directory;
}

test("native nopm path lifecycle matches", { skip }, async () => {
  const expectedDir = await makeNopmProject();
  const actualDir = await makeNopmProject();
  try {
    for (const args of [
      ["add", "path:local-utils", "--json"],
      ["lock", "--json"],
      ["tree", "--json"],
      ["install", "--json"],
      ["remove", "local-utils", "--json"],
    ]) {
      compareResult(
        `nopm ${args.join(" ")}`,
        runNodeNopm([...args, expectedDir], expectedDir),
        runNativeNopm([...args, actualDir], actualDir),
        expectedDir,
        actualDir,
      );
    }
    for (const file of ["core.json", "core.lock.json", ".astra/store/index.json"]) {
      const expected = await readFile(join(expectedDir, file), "utf8");
      const actual = await readFile(join(actualDir, file), "utf8");
      assert.equal(actual, expected, `nopm ${file}`);
    }
  } finally {
    await rm(expectedDir, { recursive: true, force: true });
    await rm(actualDir, { recursive: true, force: true });
  }
});

test("native astra nopm dispatch matches standalone nopm", { skip }, async () => {
  const directory = await mkdtemp(join(tmpdir(), "astra-native-nopm-dispatch-"));
  try {
    const standalone = spawnSync(nativeNopm(), ["version"], {
      cwd: directory,
      encoding: "utf8",
      windowsHide: true,
    });
    const dispatch = spawnSync(nativeCli(), ["nopm", "version"], {
      cwd: directory,
      encoding: "utf8",
      windowsHide: true,
    });
    compareResult("astra nopm version", standalone, dispatch, directory, directory);
  } finally {
    await rm(directory, { recursive: true, force: true });
  }
});

function withoutNodeEnv() {
  return {
    ...process.env,
    PATH: "",
    Path: "",
    ASTRA_NODE: "",
    ASTRA_TSC_BIN: "",
  };
}

test("native core runs without Node or npm on PATH", { skip }, async () => {
  const directory = await makeProject(VALID);
  try {
    for (const args of [
      ["check", directory, "--json"],
      ["fmt", directory, "--check", "--json"],
      ["ast", directory, "--json"],
      ["explain", directory, "--json"],
    ]) {
      const result = spawnSync(nativeCli(), args, {
        cwd: directory,
        env: withoutNodeEnv(),
        encoding: "utf8",
        windowsHide: true,
      });
      assert.equal(
        result.status,
        0,
        `native ${args[0]} unexpectedly needed Node/npm:\n${result.stdout}\n${result.stderr}`,
      );
    }
  } finally {
    await rm(directory, { recursive: true, force: true });
  }
});

test("native path nopm runs without Node or npm on PATH", { skip }, async () => {
  const nopmDirectory = await makeNopmProject();
  try {
    for (const args of [
      ["add", "path:local-utils", "--json"],
      ["lock", "--json"],
      ["tree", "--json"],
      ["install", "--json"],
      ["remove", "local-utils", "--json"],
    ]) {
      const result = spawnSync(nativeNopm(), [...args, nopmDirectory], {
        cwd: nopmDirectory,
        env: withoutNodeEnv(),
        encoding: "utf8",
        windowsHide: true,
      });
      assert.equal(
        result.status,
        0,
        `native nopm ${args[0]} unexpectedly needed Node/npm:\n${result.stdout}\n${result.stderr}`,
      );
    }
  } finally {
    await rm(nopmDirectory, { recursive: true, force: true });
  }
});
