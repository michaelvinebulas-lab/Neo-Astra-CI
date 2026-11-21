import test from "node:test";
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { realpathSync } from "node:fs";
import { mkdtemp, rm, symlink } from "node:fs/promises";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = join(dirname(fileURLToPath(import.meta.url)), "..", "..");
const cliDirectory = join(repoRoot, "orchestrator", "packages", "cli");

test("CLI entry runs when reached through a symlinked directory", async (t) => {
  const directory = await mkdtemp(join(tmpdir(), "neo-astra-cli-entry-"));
  t.after(() => rm(directory, { recursive: true, force: true }));

  const linkedCli = join(directory, "cli");
  await symlink(
    realpathSync(cliDirectory),
    linkedCli,
    process.platform === "win32" ? "junction" : "dir",
  );

  const result = spawnSync(
    process.execPath,
    [join(linkedCli, "dist", "index.js"), "--version"],
    { encoding: "utf8" },
  );

  assert.equal(result.status, 0, result.stderr);
  assert.match(result.stdout, /^astra \d+\.\d+\.\d+/);
});
