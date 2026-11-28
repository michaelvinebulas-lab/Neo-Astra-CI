/** Browser-safe NoPM resolution over the virtual project. */

import {
  formatLockfile,
  formatPackageId,
  parseDependencyRequest,
  type Lockfile,
  type LockfilePackage,
  type Mode,
  type PackageId,
} from "@neo-astra/nopm/core";
import { parseManifestSource } from "./manifest.js";
import type { EffectiveSettings } from "./settings.js";

export interface PackageIssue {
  readonly code: string;
  readonly message: string;
}

export interface PackageResolution {
  readonly lockfile: string;
  readonly issues: readonly PackageIssue[];
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function text(value: unknown): string | undefined {
  return typeof value === "string" ? value : undefined;
}

async function sha256(value: string): Promise<string> {
  const bytes = new TextEncoder().encode(value);
  const digest = await crypto.subtle.digest("SHA-256", bytes);
  return [...new Uint8Array(digest)].map((byte) => byte.toString(16).padStart(2, "0")).join("");
}

export function dependencySpec(alias: string, value: unknown): string {
  const direct = text(value);
  if (direct !== undefined) return direct;
  if (!isRecord(value)) return "";
  const source = text(value["source"]) ?? "npm";
  const path = text(value["path"]);
  if (path !== undefined) return `path:${path}`;
  const version = text(value["version"]);
  return version === undefined ? "" : `${source}:${alias}@${version}`;
}

function lockedPackage(
  id: PackageId,
  integrity: string | null,
  targets: readonly string[],
  resolved: string | null,
): LockfilePackage {
  return {
    id: formatPackageId(id),
    name: id.name,
    source: id.source,
    version: id.version,
    resolved,
    registry: null,
    integrity,
    targets: [...targets],
    capabilities: [],
    lifecycleScripts: false,
    dependencies: [],
    commit: id.reference,
  };
}

async function npmIntegrity(id: PackageId): Promise<{ integrity: string | null; resolved: string | null; issue?: PackageIssue }> {
  try {
    const response = await fetch(`https://registry.npmjs.org/${encodeURIComponent(id.name)}`);
    if (!response.ok) throw new Error(`registry returned ${String(response.status)}`);
    const metadata: unknown = await response.json();
    if (!isRecord(metadata)) throw new Error("registry metadata is not an object");
    const versions = isRecord(metadata["versions"]) ? metadata["versions"] : {};
    const version = id.version ?? "latest";
    const target = isRecord(versions[version]) ? versions[version] : undefined;
    const dist = target !== undefined && isRecord(target["dist"]) ? target["dist"] : undefined;
    const tarball = dist === undefined ? undefined : text(dist["tarball"]);
    if (tarball === undefined) throw new Error(`no tarball for ${version}`);
    const bytes = await fetch(tarball).then((r) => r.arrayBuffer());
    const digest = await crypto.subtle.digest("SHA-256", bytes);
    return {
      integrity: `sha256-${[...new Uint8Array(digest)].map((b) => b.toString(16).padStart(2, "0")).join("")}`,
      resolved: tarball,
    };
  } catch (error) {
    return {
      integrity: null,
      resolved: null,
      issue: {
        code: "NP0011",
        message: `npm fetch unavailable (${error instanceof Error ? error.message : String(error)}); declare-only entry kept.`,
      },
    };
  }
}

async function cargoIntegrity(id: PackageId): Promise<{ integrity: string | null; resolved: string | null; issue?: PackageIssue }> {
  const version = id.version ?? "latest";
  const url = `https://crates.io/api/v1/crates/${encodeURIComponent(id.name)}/${encodeURIComponent(version)}/download`;
  try {
    const bytes = await fetch(url).then((r) => r.arrayBuffer());
    const digest = await crypto.subtle.digest("SHA-256", bytes);
    return {
      integrity: `sha256-${[...new Uint8Array(digest)].map((b) => b.toString(16).padStart(2, "0")).join("")}`,
      resolved: url,
    };
  } catch (error) {
    return {
      integrity: null,
      resolved: null,
      issue: {
        code: "NP0011",
        message: `cargo fetch unavailable (${error instanceof Error ? error.message : String(error)}); declare-only entry kept.`,
      },
    };
  }
}

export async function resolvePackages(
  files: ReadonlyMap<string, string>,
  settings: EffectiveSettings,
): Promise<PackageResolution> {
  const issues: PackageIssue[] = [];
  const corePath = files.has("core.json") ? "core.json" : "core.toml";
  const coreText = files.get(corePath) ?? "{}";
  const data = parseManifestSource(coreText, corePath);
  const dependencies = isRecord(data["dependencies"]) ? data["dependencies"] : {};
  const targetName = settings.target === "" ? "js" : settings.target;
  const mode: Mode = settings.mode ?? "core";
  const targets: Record<string, { backend: string; mode: Mode; triple: null }> = {
    [targetName]: { backend: targetName === "ts" ? "js" : targetName, mode, triple: null },
  };

  const entries: LockfilePackage[] = [];
  for (const [alias, value] of Object.entries(dependencies)) {
    const spec = dependencySpec(alias, value);
    if (spec === "") {
      issues.push({ code: "NP0002", message: `Invalid dependency declaration for '${alias}'.` });
      continue;
    }
    let request;
    try {
      request = parseDependencyRequest(alias, spec);
    } catch (error) {
      issues.push({ code: "NP0001", message: error instanceof Error ? error.message : String(error) });
      continue;
    }
    const id = request.id;
    let integrity: string | null = null;
    let resolved: string | null = null;

    if (id.source === "path") {
      const textValue = id.path === null ? undefined : files.get(id.path);
      if (textValue === undefined) {
        issues.push({ code: "NP0003", message: `Path package '${id.path}' is not in the virtual project.` });
      } else {
        integrity = `sha256-${await sha256(textValue)}`;
      }
    } else if (id.source === "git") {
      const injected = files.get(`deps/${alias}.txt`);
      if (injected === undefined) {
        issues.push({ code: "NP0011", message: `Git package '${alias}' is declared; inject deps/${alias}.txt for integrity.` });
      } else {
        integrity = `sha256-${await sha256(injected)}`;
      }
    } else if (id.source === "npm") {
      const result = await npmIntegrity(id);
      integrity = result.integrity;
      resolved = result.resolved;
      if (result.issue !== undefined) issues.push(result.issue);
    } else if (id.source === "cargo") {
      const result = await cargoIntegrity(id);
      integrity = result.integrity;
      resolved = result.resolved;
      if (result.issue !== undefined) issues.push(result.issue);
    } else {
      issues.push({
        code: "NP0011",
        message: `${id.source} package '${alias}' is declare-only in the browser.`,
      });
    }

    entries.push(lockedPackage(id, integrity, [targetName], resolved));
  }

  const lockfile: Lockfile = {
    lockfileVersion: 1,
    generatedBy: "neo-astra-playground",
    manifestHash: `sha256:${await sha256(coreText)}`,
    compiler: { version: "1.1.2", apiVersion: "1.0" },
    semantics: {
      integers: settings.integerPolicy ?? "wrapping",
      strings: "utf8",
    },
    toolchain: null,
    targets,
    packages: entries,
    ffi: {},
  };

  return { lockfile: formatLockfile(lockfile), issues };
}
