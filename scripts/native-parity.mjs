import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";

const root = fileURLToPath(new URL("..", import.meta.url));
const result = spawnSync(
  process.execPath,
  ["--test", "orchestrator/tests/native-parity.test.mjs"],
  {
    cwd: root,
    env: { ...process.env, ASTRA_NATIVE_PARITY: "1" },
    stdio: "inherit",
  },
);

if (result.error) {
  process.stderr.write(`${result.error.message}\n`);
  process.exit(1);
}

process.exit(result.status ?? 1);
