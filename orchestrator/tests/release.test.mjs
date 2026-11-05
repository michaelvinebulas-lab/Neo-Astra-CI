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
  writeFileSync,
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
  // The real changelog may have an empty [Unreleased] right after a release.
  // Seed one entry so the fixture always exercises the section move.
  const changelogPath = join(project, "CHANGELOG.md");
  const changelog = readFileSync(changelogPath, "utf8").replace(
    "## [Unreleased]",
    "## [Unreleased]\n\n### Added\n- Fixture entry.",
  );
  writeFileSync(changelogPath, changelog, "utf8");
  return project;
}

test("release script bumps every version and moves the changelog", () => {
  const project = fixture();
  try {
    const current = JSON.parse(
      readFileSync(join(project, "package.json"), "utf8"),
    ).version;
    const [major, minor] = current.split(".").map(Number);
    const next = `${major}.${minor + 1}.0`;
    const later = `${major}.${minor + 2}.0`;
    const nextPattern = next.replace(/\./g, "\\.");

    // A version that does not move forward is refused before any write.
    run(
      ["scripts/release.mjs", current, `--root=${project}`, "--skip-cargo-lock"],
      1,
    );
    assert.equal(
      JSON.parse(readFileSync(join(project, "package.json"), "utf8")).version,
      current,
    );

    // Dry run plans but writes nothing.
    run(
      ["scripts/release.mjs", next, `--root=${project}`, "--dry-run"],
      0,
    );
    assert.equal(
      JSON.parse(readFileSync(join(project, "package.json"), "utf8")).version,
      current,
    );

    run(
      ["scripts/release.mjs", next, `--root=${project}`, "--skip-cargo-lock"],
      0,
    );
    run(["scripts/check-versions.mjs", `--root=${project}`], 0);

    assert.equal(
      JSON.parse(readFileSync(join(project, "package.json"), "utf8")).version,
      next,
    );
    const cargo = readFileSync(join(project, "Cargo.toml"), "utf8");
    assert.match(
      cargo,
      new RegExp(`^\\[workspace\\.package\\][\\s\\S]*?^version\\s*=\\s*"${nextPattern}"`, "m"),
    );
    const lockfile = JSON.parse(
      readFileSync(join(project, "package-lock.json"), "utf8"),
    );
    assert.equal(lockfile.version, next);
    assert.equal(lockfile.packages[""].version, next);
    assert.equal(
      lockfile.packages["orchestrator/packages/cli"].version,
      next,
    );
    assert.match(
      readFileSync(join(project, "orchestrator/packages/cli/src/index.ts"), "utf8"),
      new RegExp(`const VERSION = "${nextPattern}";`),
    );
    assert.match(
      readFileSync(join(project, "orchestrator/packages/nopm/src/cli.ts"), "utf8"),
      new RegExp(`const VERSION = "${nextPattern}";`),
    );

    const changelog = readFileSync(join(project, "CHANGELOG.md"), "utf8");
    assert.ok(
      changelog.indexOf("## [Unreleased]") <
        changelog.indexOf(`## [${next}]`),
    );
    assert.match(
      changelog,
      new RegExp(`## \\[${nextPattern}\\] - \\d{4}-\\d{2}-\\d{2}`),
    );
    assert.ok(changelog.includes("### Added"));

    // The moved entries leave a fresh empty [Unreleased], so a second bump
    // without new entries is refused.
    run(
      ["scripts/release.mjs", later, `--root=${project}`, "--skip-cargo-lock"],
      1,
    );
  } finally {
    rmSync(project, { recursive: true, force: true });
  }
});
