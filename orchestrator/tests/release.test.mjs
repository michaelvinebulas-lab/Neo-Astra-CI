import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import {
  copyFileSync,
  existsSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  readdirSync,
  rmSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";

const root = fileURLToPath(new URL("../..", import.meta.url));

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

function fixture() {
  const project = mkdtempSync(join(tmpdir(), "astra-release-test-"));
  const copy = (relative) => {
    const target = join(project, relative);
    mkdirSync(dirname(target), { recursive: true });
    copyFileSync(join(root, relative), target);
  };
  for (const file of [
    "package.json",
    "Cargo.toml",
    "package-lock.json",
    "CHANGELOG.md",
  ]) {
    copy(file);
  }
  for (const name of readdirSync(join(root, "orchestrator/packages"))) {
    const relative = `orchestrator/packages/${name}/package.json`;
    if (existsSync(join(root, relative))) {
      copy(relative);
    }
  }
  for (const file of [
    "orchestrator/packages/cli/src/index.ts",
    "orchestrator/packages/nopm/src/cli.ts",
  ]) {
    copy(file);
  }
  return project;
}

test("release script bumps every version and moves the changelog", () => {
  const project = fixture();
  try {
    // A version that does not move forward is refused before any write.
    run(
      ["scripts/release.mjs", "0.1.0", `--root=${project}`, "--skip-cargo-lock"],
      1,
    );
    assert.equal(
      JSON.parse(readFileSync(join(project, "package.json"), "utf8")).version,
      "0.1.0",
    );

    // Dry run plans but writes nothing.
    run(
      ["scripts/release.mjs", "1.0.0", `--root=${project}`, "--dry-run"],
      0,
    );
    assert.equal(
      JSON.parse(readFileSync(join(project, "package.json"), "utf8")).version,
      "0.1.0",
    );

    run(
      ["scripts/release.mjs", "1.0.0", `--root=${project}`, "--skip-cargo-lock"],
      0,
    );
    run(["scripts/check-versions.mjs", `--root=${project}`], 0);

    assert.equal(
      JSON.parse(readFileSync(join(project, "package.json"), "utf8")).version,
      "1.0.0",
    );
    const cargo = readFileSync(join(project, "Cargo.toml"), "utf8");
    assert.match(
      cargo,
      /^\[workspace\.package\][\s\S]*?^version\s*=\s*"1\.0\.0"/m,
    );
    const lockfile = JSON.parse(
      readFileSync(join(project, "package-lock.json"), "utf8"),
    );
    assert.equal(lockfile.version, "1.0.0");
    assert.equal(lockfile.packages[""].version, "1.0.0");
    assert.equal(
      lockfile.packages["orchestrator/packages/cli"].version,
      "1.0.0",
    );
    assert.match(
      readFileSync(join(project, "orchestrator/packages/cli/src/index.ts"), "utf8"),
      /const VERSION = "1\.0\.0";/,
    );
    assert.match(
      readFileSync(join(project, "orchestrator/packages/nopm/src/cli.ts"), "utf8"),
      /const VERSION = "1\.0\.0";/,
    );

    const changelog = readFileSync(join(project, "CHANGELOG.md"), "utf8");
    assert.ok(
      changelog.indexOf("## [Unreleased]") < changelog.indexOf("## [1.0.0]"),
    );
    assert.match(changelog, /## \[1\.0\.0\] - \d{4}-\d{2}-\d{2}/);
    assert.ok(changelog.includes("### Added"));

    // The moved entries leave a fresh empty [Unreleased], so a second bump
    // without new entries is refused.
    run(
      ["scripts/release.mjs", "1.1.0", `--root=${project}`, "--skip-cargo-lock"],
      1,
    );
  } finally {
    rmSync(project, { recursive: true, force: true });
  }
});
