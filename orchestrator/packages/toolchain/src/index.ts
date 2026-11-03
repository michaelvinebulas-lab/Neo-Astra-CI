export class ToolchainError extends Error {
  readonly code: string;
  constructor(code: string, message: string) {
    super(message);
    this.name = "ToolchainError";
    this.code = code;
  }
}

export interface ToolchainPolicy {
  readonly request: "auto" | string;
}

export type ToolchainRung = "env" | "cache" | "path";

export interface ToolchainResolution {
  readonly rung: ToolchainRung;
  readonly id: string;
  readonly family: string;
  readonly path: string;
  readonly version: string | null;
}

export interface ToolchainProbe {
  readonly exists: (path: string) => boolean;
  readonly run: (command: string, args: readonly string[]) => { status: number | null; stdout: string; stderr: string };
  readonly readFileSync: (path: string) => string;
  readonly readdir: (path: string) => string[];
}

export interface ToolchainAttempt {
  readonly rung: ToolchainRung;
  readonly detail: string;
}

export interface ResolveOptions {
  readonly projectRoot: string;
  readonly policy: ToolchainPolicy;
  readonly env: { readonly ASTRA_CC?: string };
  readonly probe: ToolchainProbe;
  /** Records every rung the resolver actually tries, in order. */
  readonly onAttempt?: (attempt: ToolchainAttempt) => void;
}

const FAMILIES = new Set(["clang", "gcc", "zig", "cc"]);

function isRecord(v: unknown): v is Record<string, unknown> {
  return typeof v === "object" && v !== null && !Array.isArray(v);
}

export function parseToolchainPolicy(buildData: unknown): ToolchainPolicy {
  let raw: unknown = "auto";
  if (buildData !== undefined) {
    if (!isRecord(buildData)) {
      throw new ToolchainError("NS9101", "build manifest must be an object");
    }
    if (Object.prototype.hasOwnProperty.call(buildData, "toolchain")) {
      const tc = buildData["toolchain"];
      if (!isRecord(tc)) {
        throw new ToolchainError("NS9101", "toolchain must be an object");
      }
      if (Object.prototype.hasOwnProperty.call(tc, "c")) {
        raw = tc["c"];
      }
    }
  }
  if (raw === undefined) raw = "auto";
  if (typeof raw !== "string") {
    throw new ToolchainError("NS9101", "toolchain.c must be a string");
  }
  const request = raw.trim();
  if (request === "") {
    throw new ToolchainError("NS9101", "toolchain.c must not be empty");
  }
  if (request === "auto") {
    return { request: "auto" };
  }
  // Internal whitespace is never part of a pin id. Leading/trailing
  // whitespace around the whole value was removed above.
  if (/\s/.test(request)) {
    throw new ToolchainError("NS9101", "toolchain.c must not contain whitespace");
  }
  const parts = request.split("@");
  if (parts.length > 2) {
    throw new ToolchainError("NS9101", "toolchain.c must not contain multiple '@'");
  }
  const family = parts[0] ?? "";
  const version = parts.length === 2 ? (parts[1] ?? "") : "";
  if (!FAMILIES.has(family)) {
    throw new ToolchainError("NS9101", `unknown toolchain family '${family}'`);
  }
  if (parts.length === 2 && version === "") {
    throw new ToolchainError("NS9101", "toolchain.c version must not be empty");
  }
  return { request };
}

function familyFromBinary(name: string): string {
  const base = name.replace(/\.exe$/i, "").toLowerCase();
  if (base === "clang") return "clang";
  if (base === "gcc" || base === "cc") return "gcc";
  if (base === "zig") return "zig";
  return base;
}

/**
 * Returns the candidate name when it answers `--version`. A missing command
 * and a command that rejects `--version` are both unavailable for resolution.
 */
function findOnPath(name: string, probe: ToolchainProbe): string | null {
  const result = probe.run(name, ["--version"]);
  return result.status === 0 ? name : null;
}

/** The first line of `--version`, or null when the binary does not answer. */
function versionOf(probe: ToolchainProbe, bin: string): string | null {
  const result = probe.run(bin, ["--version"]);
  if (result.status !== 0) return null;
  const line = result.stdout.trim().split(/\r?\n/)[0]?.trim() ?? "";
  return line === "" ? null : line;
}

function record(options: ResolveOptions, rung: ToolchainRung, detail: string): void {
  options.onAttempt?.({ rung, detail });
}

interface CacheResult {
  readonly resolution: ToolchainResolution | null;
  readonly reason: string;
  readonly canFallBack: boolean;
}

/**
 * A cache entry is authoritative when its marker exists: the marker's id,
 * family, and (for versioned pins) version must match. Only a *missing*
 * marker may fall back to PATH for a bare family pin.
 */
function lookupCache(projectRoot: string, id: string, probe: ToolchainProbe): CacheResult {
  const at = id.indexOf("@");
  const family = at >= 0 ? id.slice(0, at) : id;
  const versioned = at >= 0;
  const cacheDir = `${projectRoot}/.tools/toolchain/${id}`;
  const markerPath = `${cacheDir}/toolchain.json`;
  if (!probe.exists(markerPath)) {
    return {
      resolution: null,
      reason: `no cache marker at ${markerPath}`,
      canFallBack: !versioned,
    };
  }
  let marker: unknown;
  try {
    marker = JSON.parse(probe.readFileSync(markerPath));
  } catch {
    return {
      resolution: null,
      reason: `invalid cache marker at ${markerPath}`,
      canFallBack: false,
    };
  }
  if (!isRecord(marker)) {
    return {
      resolution: null,
      reason: `invalid cache marker at ${markerPath}`,
      canFallBack: false,
    };
  }
  if (marker["id"] !== id) {
    return {
      resolution: null,
      reason: `cache marker id mismatch: expected ${id}, got ${String(marker["id"])}`,
      canFallBack: false,
    };
  }
  if (marker["family"] !== family) {
    return {
      resolution: null,
      reason: `cache marker family mismatch: expected ${family}, got ${String(marker["family"])}`,
      canFallBack: false,
    };
  }
  const markerVersion = marker["version"];
  if (typeof markerVersion !== "string") {
    return {
      resolution: null,
      reason: `cache marker has no version at ${markerPath}`,
      canFallBack: false,
    };
  }
  if (versioned && markerVersion !== id.slice(at + 1)) {
    return {
      resolution: null,
      reason: `cache marker version mismatch: expected ${id.slice(at + 1)}, got ${markerVersion}`,
      canFallBack: false,
    };
  }
  const candidates = [
    `${cacheDir}/bin/${family}`,
    `${cacheDir}/bin/${family}.exe`,
    `${cacheDir}/${family}`,
    `${cacheDir}/${family}.exe`,
  ];
  const bin = candidates.find((candidate) => probe.exists(candidate));
  if (bin === undefined) {
    return {
      resolution: null,
      reason: `cache marker present but binary missing for ${family} in ${cacheDir}`,
      canFallBack: false,
    };
  }
  return {
    resolution: { rung: "cache", id, family, path: bin, version: markerVersion },
    reason: "",
    canFallBack: false,
  };
}

export function resolveToolchain(options: ResolveOptions): ToolchainResolution {
  const { projectRoot, policy, env, probe } = options;

  // ASTRA_CC (env rung) wins over the manifest policy and never falls through.
  const astraCc = env.ASTRA_CC?.trim();
  if (astraCc) {
    record(options, "env", `ASTRA_CC=${astraCc}`);
    const exists = probe.exists(astraCc);
    const path = exists ? astraCc : findOnPath(astraCc, probe);
    if (path === null) {
      throw new ToolchainError(
        "NS9103",
        `ASTRA_CC does not exist and is not on PATH: ${astraCc}`,
      );
    }
    const version = versionOf(probe, path);
    if (version === null) {
      throw new ToolchainError("NS9103", `ASTRA_CC does not answer --version: ${astraCc}`);
    }
    const family = familyFromBinary(path.split(/[/\\]/).pop() ?? path);
    return { rung: "env", id: family, family, path, version };
  }

  // Pinned id: cache first, then PATH only for a bare family pin whose marker
  // is missing.
  if (policy.request !== "auto") {
    const id = policy.request;
    const cache = lookupCache(projectRoot, id, probe);
    if (cache.resolution !== null) {
      record(options, "cache", `resolved ${cache.resolution.path}`);
      return cache.resolution;
    }
    record(options, "cache", cache.reason);
    if (!cache.canFallBack) {
      throw new ToolchainError("NS9102", cache.reason);
    }
    const family = id;
    const pathBin = findOnPath(family, probe);
    if (pathBin !== null) {
      const version = versionOf(probe, pathBin);
      if (version !== null) {
        record(options, "path", `resolved ${pathBin}`);
        return { rung: "path", id: family, family, path: pathBin, version };
      }
      record(options, "path", `${family}: --version did not answer`);
    } else {
      record(options, "path", `${family}: not on PATH`);
    }
    throw new ToolchainError("NS9102", `no cached or PATH toolchain for ${family}`);
  }

  // auto: first pass is the hosted probe, in candidate order. The second pass
  // accepts the first candidate that only answers --version.
  const candidates = ["clang", "gcc", "cc"];
  for (const candidate of candidates) {
    const bin = findOnPath(candidate, probe);
    if (bin === null) {
      record(options, "path", `${candidate}: not on PATH`);
      continue;
    }
    const hosted = probe.run(bin, ["-std=c17", "-fsyntax-only", "-x", "c", "-"]);
    const version = versionOf(probe, bin);
    record(
      options,
      "path",
      `${candidate}: hosted probe ${hosted.status === 0 ? "passed" : "failed"}, --version ${
        version === null ? "failed" : "ok"
      }`,
    );
    if (hosted.status === 0 && version !== null) {
      const family = familyFromBinary(candidate);
      return { rung: "path", id: family, family, path: bin, version };
    }
  }
  for (const candidate of candidates) {
    const bin = findOnPath(candidate, probe);
    if (bin === null) continue;
    const version = versionOf(probe, bin);
    if (version === null) continue;
    record(options, "path", `${candidate}: fallback --version responder`);
    const family = familyFromBinary(candidate);
    return { rung: "path", id: family, family, path: bin, version };
  }
  throw new ToolchainError("NS9102", "no toolchain found on PATH");
}

export interface DoctorResult {
  readonly attempts: readonly ToolchainAttempt[];
  readonly winner: ToolchainResolution | null;
  readonly error: string | null;
}

export function doctor(
  projectRoot: string,
  policy: ToolchainPolicy,
  env: { readonly ASTRA_CC?: string },
  probe: ToolchainProbe,
): DoctorResult {
  const attempts: ToolchainAttempt[] = [];
  let winner: ToolchainResolution | null = null;
  let error: string | null = null;
  try {
    winner = resolveToolchain({
      projectRoot,
      policy,
      env,
      probe,
      onAttempt: (attempt) => attempts.push(attempt),
    });
  } catch (e) {
    error = e instanceof ToolchainError ? `${e.code}: ${e.message}` : e instanceof Error ? e.message : String(e);
  }
  return { attempts, winner, error };
}
