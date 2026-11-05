//! TypeScript backend for Neo Astra.
//!
//! The backend consumes the same serialized typed module as `codegen-js` and
//! runs it through the same shared emission pipeline (`generateModule`), with
//! `backend: "ts"` adding explicit type annotations, body-level type
//! declarations, and typed runtime helpers. It never parses `.ns` source and
//! never forks runtime semantics from the JavaScript backend.

import { spawnSync } from "node:child_process";
import { existsSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { createRequire } from "node:module";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { generateModule } from "@neo-astra/codegen-js";
import type {
  CodegenDiagnostic,
  CodegenRequest,
  CodegenResult,
  TypeScriptTranspileOutcome,
} from "@neo-astra/codegen-js";

/**
 * The one pinned TypeScript compiler version used to transpile `@script:ts`
 * bodies. A missing or mismatched install is a hard error, not a fallback.
 */
export const PINNED_TYPESCRIPT_VERSION = "7.0.2";

/** Input accepted by `generateTypeScript`. Same typed module as `codegen-js`. */
export interface TypeScriptCodegenRequest extends Omit<CodegenRequest, "backend"> {}

export type { CodegenDiagnostic, CodegenRequest, CodegenResult };
export { CodegenRequestError } from "@neo-astra/codegen-js";

const requireFromCodegenTs = createRequire(import.meta.url);

/**
 * Checks an installed `typescript` version against the pin.
 *
 * Returns `undefined` when the install matches `PINNED_TYPESCRIPT_VERSION`
 * exactly and a human-readable reason otherwise. There is no semver range and
 * no best-effort fallback: a mismatched compiler is a hard error so that the
 * same `.ns` source cannot transpile differently across machines.
 */
export function pinnedTypeScriptVersionError(
  installedVersion: string,
): string | undefined {
  if (installedVersion === PINNED_TYPESCRIPT_VERSION) {
    return undefined;
  }
  return `typescript@${PINNED_TYPESCRIPT_VERSION} is required, but typescript@${installedVersion} is installed`;
}

/**
 * Resolves the pinned TypeScript native compiler.
 *
 * TypeScript 7 ships the compiler as a platform binary
 * (`@typescript/typescript-<platform>-<arch>/lib/tsc[.exe]`). A packaged CLI
 * carries a `typescript/` directory next to the executable; a source install
 * resolves the optional platform package from `node_modules`. Neither path
 * needs a Node launcher, so `@script:ts` works in the compiled CLI.
 */
function resolveNativeTypeScript():
  | { readonly path: string; readonly error?: undefined }
  | { readonly path?: undefined; readonly error: string } {
  const executable = process.platform === "win32" ? "tsc.exe" : "tsc";
  const packagedDir = join(dirname(process.execPath), "typescript");
  const explicit = process.env["ASTRA_TSC_BIN"];
  let installedVersion: string | undefined;
  let tscPath: string | undefined;

  const readVersion = (packageJsonPath: string): string | undefined => {
    try {
      const value = JSON.parse(readFileSync(packageJsonPath, "utf8")) as {
        version?: unknown;
      };
      return typeof value.version === "string" ? value.version : undefined;
    } catch {
      return undefined;
    }
  };

  try {
    const packageJsonPath = requireFromCodegenTs.resolve("typescript/package.json");
    installedVersion = readVersion(packageJsonPath);
    const platformPackage = `@typescript/typescript-${process.platform}-${process.arch}`;
    const platformJson = requireFromCodegenTs.resolve(
      `${platformPackage}/package.json`,
    );
    tscPath = join(dirname(platformJson), "lib", executable);
  } catch {
    // A packaged CLI and an explicit override are resolved below.
  }

  const packagedJson = join(packagedDir, "package.json");
  const packagedVersion = existsSync(packagedJson)
    ? readVersion(packagedJson)
    : undefined;
  if (packagedVersion !== undefined) {
    installedVersion = packagedVersion;
  }
  const packagedTsc = join(packagedDir, executable);
  if (existsSync(packagedTsc)) {
    tscPath = packagedTsc;
  }
  if (explicit !== undefined && explicit.length > 0) {
    tscPath = explicit;
    if (installedVersion === undefined) {
      const version = spawnSync(explicit, ["--version"], {
        encoding: "utf8",
        windowsHide: true,
      });
      installedVersion =
        version.status === 0
          ? (version.stdout.trim().replace(/^Version\s+/i, "") || undefined)
          : undefined;
    }
  }

  if (installedVersion === undefined) {
    return {
      error: `the pinned TypeScript compiler (typescript@${PINNED_TYPESCRIPT_VERSION}) is not installed`,
    };
  }
  const versionError = pinnedTypeScriptVersionError(installedVersion);
  if (versionError !== undefined) {
    return { error: versionError };
  }
  if (tscPath === undefined || !existsSync(tscPath)) {
    return {
      error: `the pinned TypeScript native compiler (typescript@${PINNED_TYPESCRIPT_VERSION}) is not installed; install the platform package or set ASTRA_TSC_BIN`,
    };
  }
  return { path: tscPath };
}

/**
 * Transpiles one `@script:ts` body with the pinned TypeScript compiler.
 *
 * The input is the script body padded with leading blank lines so the emitted
 * source map reports `.ns`-absolute line numbers. The caller composes the
 * artifact map from the returned map; this function only runs the pinned
 * compiler. A missing or mismatched `typescript` install, a failed `tsc`
 * invocation, or a syntactically invalid body all return `{ ok: false }` so
 * the build hard-errors instead of falling back.
 */
export function transpileTypeScriptBody(
  paddedSource: string,
): TypeScriptTranspileOutcome {
  const tsc = resolveNativeTypeScript();
  if (tsc.error !== undefined) {
    return { ok: false, message: tsc.error };
  }

  const directory = mkdtempSync(join(tmpdir(), "astra-script-ts-"));
  try {
    const inputPath = join(directory, "script.ts");
    const outDir = join(directory, "out");
    writeFileSync(inputPath, paddedSource, "utf8");
    // `--noCheck` transpiles without type-checking: the foreign body is
    // opaque to Nova and may legitimately use host globals that a strict
    // library check would reject. Syntax errors are still hard errors.
    const result = spawnSync(
      tsc.path,
      [
        "--ignoreConfig",
        "--noCheck",
        "--target",
        "es2022",
        "--module",
        "esnext",
        "--moduleResolution",
        "bundler",
        "--sourceMap",
        "--outDir",
        outDir,
        "script.ts",
      ],
      { cwd: directory, encoding: "utf8" },
    );
    const output = `${result.stdout ?? ""}${result.stderr ?? ""}`;
    const diagnostics = output
      .split(/\r?\n/)
      .filter((line) => line.includes("error TS"));
    if (result.status !== 0 || diagnostics.length > 0) {
      return {
        ok: false,
        message:
          diagnostics.length > 0
            ? diagnostics.join("\n")
            : `tsc exited with status ${String(result.status)}`,
      };
    }
    const code = readFileSync(join(outDir, "script.js"), "utf8");
    let sourceMap: string;
    try {
      sourceMap = readFileSync(join(outDir, "script.js.map"), "utf8");
    } catch {
      return { ok: false, message: "the pinned TypeScript compiler emitted no source map" };
    }
    return { ok: true, code, sourceMap };
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    return { ok: false, message: `failed to run the pinned TypeScript compiler: ${message}` };
  } finally {
    rmSync(directory, { recursive: true, force: true });
  }
}

/**
 * Generates strict TypeScript ESM for one typed module.
 *
 * The output is the JavaScript backend's emission with explicit types layered
 * on top: typed function signatures, annotated bindings, `interface`/`type`
 * declarations for local nominal types, and `tsc --strict`-clean helpers —
 * identical runtime semantics, one pipeline, one source of truth.
 *
 * `@script:ts` bodies are transpiled with the pinned TypeScript compiler
 * (`PINNED_TYPESCRIPT_VERSION`); a missing or mismatched install is a hard
 * error. Callers may override `transpileTypeScript` for testing or for
 * supplying their own pinned toolchain.
 */
export function generateTypeScript(
  request: TypeScriptCodegenRequest,
): CodegenResult {
  return generateModule({
    transpileTypeScript: transpileTypeScriptBody,
    ...request,
    backend: "ts",
  });
}
