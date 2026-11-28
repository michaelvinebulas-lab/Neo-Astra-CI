//! Node-specific NoPM helpers. The portable resolver model lives in `core.ts`,
//! which has no Node imports and is safe for the browser playground.

import { createHash } from "node:crypto";
import { readFileSync, renameSync, rmSync, writeFileSync } from "node:fs";

import {
  LOCKFILE_NAME,
  NopmError,
  formatLockfile,
  parseLockfile,
  type Lockfile,
} from "./core.js";

export * from "./core.js";

/** Computes the stable manifest hash recorded in `core.lock.json`. */
export function hashManifestSource(source: string): string {
  const normalized = source.replace(/^\uFEFF/, "").replace(/\r\n/g, "\n");
  return `sha256:${createHash("sha256").update(normalized, "utf8").digest("hex")}`;
}

/** Returns whether a lockfile was generated from the supplied manifest source. */
export function isLockfileCurrent(lockfile: Lockfile, manifestSource: string): boolean {
  return lockfile.manifestHash === hashManifestSource(manifestSource);
}

/** Throws `NP0009` when the lockfile is stale for the supplied manifest. */
export function assertLockfileCurrent(
  lockfile: Lockfile,
  manifestSource: string,
): Lockfile {
  const expected = hashManifestSource(manifestSource);
  if (lockfile.manifestHash !== expected) {
    throw new NopmError(
      "NP0009",
      `Lockfile is stale. Expected ${expected}, found ${lockfile.manifestHash}.`,
      LOCKFILE_NAME,
    );
  }
  return lockfile;
}

/** Reads and validates a lockfile from disk. */
export function readLockfileFile(path: string): Lockfile {
  let source: string;
  try {
    source = readFileSync(path, "utf8");
  } catch (error) {
    if (typeof error === "object" && error !== null && "code" in error && error.code === "ENOENT") {
      throw new NopmError("NP0009", `Lockfile '${path}' does not exist.`, path, {
        cause: error,
      });
    }
    throw new NopmError("NP0008", `Unable to read lockfile '${path}'.`, path, {
      cause: error,
    });
  }
  return parseLockfile(source);
}

/** Writes a lockfile atomically in canonical format. */
export function writeLockfileFile(path: string, lockfile: Lockfile): void {
  const source = formatLockfile(lockfile);
  const temporary = `${path}.tmp-${process.pid}-${Date.now()}`;
  try {
    writeFileSync(temporary, source, "utf8");
    renameSync(temporary, path);
  } catch (error) {
    try {
      rmSync(temporary, { force: true });
    } catch {
      // Keep the original write error.
    }
    throw new NopmError("NP0010", `Unable to write lockfile '${path}'.`, path, {
      cause: error,
    });
  }
}
