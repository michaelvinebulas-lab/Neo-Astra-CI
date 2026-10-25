import { spawnSync } from "node:child_process";
import { statSync } from "node:fs";
import { delimiter, dirname, isAbsolute, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

/** Compiler protocol version consumed by this client. */
export const COMPILER_API_VERSION = "1.0";

/** Neo Astra capability modes understood by the compiler boundary. */
export type Mode = "core" | "app" | "web" | "system";

/** A source unit sent to the Rust compiler. */
export interface SourceUnit {
  readonly path: string;
  readonly text: string;
}

/** Byte span reported by the Rust lexer and parser. */
export interface DiagnosticSpan {
  readonly start: number;
  readonly end: number;
}

/** Structured compiler diagnostic returned over JSON-RPC. */
export interface CompilerDiagnostic {
  readonly code: string;
  readonly severity: "error" | "warning" | "note" | "help";
  readonly message: string;
  readonly span: DiagnosticSpan | null;
  readonly hint: string | null;
}

/** Canonical-format result returned by the Rust compiler. */
export interface FormatResponse {
  readonly apiVersion: string;
  readonly source: string | null;
  readonly diagnostics: readonly CompilerDiagnostic[];
}

/** Integer arithmetic policy selected by the manifest. */
export type IntegerPolicy = "wrapping" | "safe-checked" | "arbitrary";

/** Checked expression type keyed by source span. */
export interface TypeAnnotationIr {
  readonly span: DiagnosticSpan;
  /** Serialized semantic `astra_core::types::Type`. */
  readonly ty: unknown;
}

/** Typed module payload returned by `compile`. */
export interface ModuleIr {
  /** Canonical module identity used by imports. */
  readonly key: string;
  /** Display path used by diagnostics. */
  readonly path: string;
  /** Serialized source AST. */
  readonly ast: unknown;
  /** Serialized resolved HIR, including imports and exports. */
  readonly hir: unknown;
  /** Checked expression annotations in source order. */
  readonly annotations: readonly TypeAnnotationIr[];
  /** Spans of manual-memory scopes, rejected by non-native backends. */
  readonly manualMemoryScopes: readonly DiagnosticSpan[];
  readonly integerPolicy: IntegerPolicy;
}

/** Parameters for `CompilerClient.compile`. */
export interface CompileRequest {
  readonly source: SourceUnit;
  readonly target?: "js" | "ts";
  readonly mode?: Mode;
  readonly integerPolicy?: IntegerPolicy;
}

/** Result returned by `CompilerClient.compile`. */
export interface CompileResponse {
  readonly apiVersion: string;
  readonly program: { readonly name: string } | null;
  readonly ir?: ModuleIr;
  readonly diagnostics: readonly CompilerDiagnostic[];
  readonly metadata: Readonly<Record<string, unknown>>;
  readonly artifacts: readonly unknown[];
}

/** Parameters for a canonical-format request. */
export interface FormatRequest {
  readonly source: SourceUnit;
  readonly target?: string;
  /** Formatting is target independent; an omitted mode is accepted. */
  readonly mode?: Mode;
}

/** Options used to locate and invoke the native compiler binary. */
export interface CompilerClientOptions {
  /** Explicit binary path. Falls back to ASTRA_COMPILER_BIN, then discovery. */
  readonly binaryPath?: string;
  /** Working directory used for discovery and process execution. */
  readonly cwd?: string;
  /** Maximum time to wait for one compiler request, in milliseconds. */
  readonly timeoutMs?: number;
}

/** Error codes owned by the TypeScript compiler client. */
export type CompilerClientErrorCode = "NS9003" | "NS9004" | "NS9005" | "NS9006";

/** A native compiler discovery, process, or protocol failure. */
export class CompilerClientError extends Error {
  readonly code: CompilerClientErrorCode;

  constructor(code: CompilerClientErrorCode, message: string, options?: ErrorOptions) {
    super(message, options);
    this.name = "CompilerClientError";
    this.code = code;
  }
}

const BINARY_NAME = process.platform === "win32" ? "astra-compiler.exe" : "astra-compiler";

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function isFile(path: string): boolean {
  try {
    return statSync(path).isFile();
  } catch {
    return false;
  }
}

function binaryNames(name: string): readonly string[] {
  if (process.platform === "win32" && !name.toLowerCase().endsWith(".exe")) {
    return [name, `${name}.exe`];
  }

  return [name];
}

function resolveBinaryCandidate(value: string, cwd: string): string | undefined {
  const candidate = isAbsolute(value) ? value : resolve(cwd, value);

  for (const name of binaryNames(candidate)) {
    if (isFile(name)) {
      return name;
    }
  }

  if (!value.includes("/") && !value.includes("\\")) {
    const pathValue = process.env["PATH"];
    if (pathValue !== undefined) {
      for (const directory of pathValue.split(delimiter)) {
        if (directory.length === 0) {
          continue;
        }

        for (const name of binaryNames(value)) {
          const path = join(directory, name);
          if (isFile(path)) {
            return path;
          }
        }
      }
    }
  }

  return undefined;
}

/**
 * Locates the native `astra-compiler` binary.
 *
 * Discovery order is: explicit option, ASTRA_COMPILER_BIN, `target/debug` and
 * `target/release` under the working directory or one of its ancestors, then
 * the same directories under the workspace containing this package.
 */
export function findCompilerBinary(
  options: Pick<CompilerClientOptions, "binaryPath" | "cwd"> = {},
): string {
  const cwd = resolve(options.cwd ?? process.cwd());
  const explicit = options.binaryPath ?? process.env["ASTRA_COMPILER_BIN"];

  if (explicit !== undefined && explicit.length > 0) {
    const found = resolveBinaryCandidate(explicit, cwd);
    if (found !== undefined) {
      return found;
    }

    throw new CompilerClientError(
      "NS9003",
      `Compiler binary '${explicit}' was not found. Set ASTRA_COMPILER_BIN or run "npm run bootstrap".`,
    );
  }

  const roots = new Set<string>();
  const addAncestors = (start: string): void => {
    let current = resolve(start);

    for (;;) {
      roots.add(current);
      const parent = dirname(current);
      if (parent === current) {
        return;
      }
      current = parent;
    }
  };

  addAncestors(cwd);

  try {
    addAncestors(fileURLToPath(new URL("../../../../", import.meta.url)));
  } catch {
    // The package location is only a hint; keep the working-directory search.
  }

  for (const root of roots) {
    for (const profile of ["debug", "release"]) {
      const candidate = join(root, "target", profile, BINARY_NAME);
      if (isFile(candidate)) {
        return candidate;
      }
    }
  }

  throw new CompilerClientError(
    "NS9003",
    `Compiler binary '${BINARY_NAME}' was not found. Build it with "cargo build -p astra-compiler-api --bin astra-compiler" or set ASTRA_COMPILER_BIN.`,
  );
}

function parseDiagnostic(value: unknown): CompilerDiagnostic {
  if (!isRecord(value)) {
    throw new CompilerClientError("NS9005", "Compiler returned an invalid diagnostic.");
  }

  const { code, severity, message } = value;
  const spanValue = value["span"] ?? null;
  const hintValue = value["hint"] ?? null;

  if (
    typeof code !== "string" ||
    (severity !== "error" &&
      severity !== "warning" &&
      severity !== "note" &&
      severity !== "help") ||
    typeof message !== "string" ||
    (hintValue !== null && typeof hintValue !== "string")
  ) {
    throw new CompilerClientError("NS9005", "Compiler returned an invalid diagnostic.");
  }

  let span: DiagnosticSpan | null = null;
  if (spanValue !== null) {
    if (
      !isRecord(spanValue) ||
      typeof spanValue["start"] !== "number" ||
      typeof spanValue["end"] !== "number"
    ) {
      throw new CompilerClientError("NS9005", "Compiler returned an invalid diagnostic span.");
    }

    span = { start: spanValue["start"], end: spanValue["end"] };
  }

  return {
    code,
    severity,
    message,
    span,
    hint: hintValue,
  };
}

function parseFormatResponse(value: unknown): FormatResponse {
  if (!isRecord(value)) {
    throw new CompilerClientError("NS9005", "Compiler returned an invalid format result.");
  }

  const { apiVersion, source, diagnostics } = value;
  if (typeof apiVersion !== "string" || apiVersion !== COMPILER_API_VERSION) {
    throw new CompilerClientError(
      "NS9005",
      `Compiler returned an incompatible format result; expected API ${COMPILER_API_VERSION}.`,
    );
  }

  if ((source !== null && typeof source !== "string") || !Array.isArray(diagnostics)) {
    throw new CompilerClientError("NS9005", "Compiler returned an invalid format result.");
  }

  return {
    apiVersion,
    source,
    diagnostics: diagnostics.map(parseDiagnostic),
  };
}

function parseModuleIr(value: unknown): ModuleIr {
  if (!isRecord(value)) {
    throw new CompilerClientError("NS9005", "Compiler returned an invalid typed module.");
  }

  const { key, path, ast, hir, annotations, manualMemoryScopes, integerPolicy } = value;
  if (
    typeof key !== "string" ||
    typeof path !== "string" ||
    !isRecord(ast) ||
    !isRecord(hir) ||
    !Array.isArray(annotations) ||
    !Array.isArray(manualMemoryScopes) ||
    (integerPolicy !== "wrapping" &&
      integerPolicy !== "safe-checked" &&
      integerPolicy !== "arbitrary")
  ) {
    throw new CompilerClientError("NS9005", "Compiler returned an invalid typed module.");
  }

  const parsedScopes = manualMemoryScopes.map((scope): DiagnosticSpan => {
    if (!isRecord(scope) || typeof scope["start"] !== "number" || typeof scope["end"] !== "number") {
      throw new CompilerClientError(
        "NS9005",
        "Compiler returned an invalid manual-memory scope.",
      );
    }
    return { start: scope["start"], end: scope["end"] };
  });

  return {
    key,
    path,
    ast,
    hir,
    annotations: annotations.map((annotation): TypeAnnotationIr => {
      if (!isRecord(annotation) || !isRecord(annotation["span"])) {
        throw new CompilerClientError(
          "NS9005",
          "Compiler returned an invalid type annotation.",
        );
      }
      const span = annotation["span"];
      if (typeof span["start"] !== "number" || typeof span["end"] !== "number") {
        throw new CompilerClientError(
          "NS9005",
          "Compiler returned an invalid type annotation span.",
        );
      }
      return {
        span: { start: span["start"], end: span["end"] },
        ty: annotation["ty"] as TypeAnnotationIr["ty"],
      };
    }),
    manualMemoryScopes: parsedScopes,
    integerPolicy,
  };
}

function parseCompileResponse(value: unknown): CompileResponse {
  if (!isRecord(value)) {
    throw new CompilerClientError("NS9005", "Compiler returned an invalid compile result.");
  }

  const { apiVersion, program, diagnostics, metadata, artifacts } = value;
  const ir = value["ir"];
  if (
    typeof apiVersion !== "string" ||
    apiVersion !== COMPILER_API_VERSION ||
    !Array.isArray(diagnostics) ||
    !isRecord(metadata) ||
    !Array.isArray(artifacts)
  ) {
    throw new CompilerClientError(
      "NS9005",
      `Compiler returned an incompatible compile result; expected API ${COMPILER_API_VERSION}.`,
    );
  }

  const parsedProgram =
    program === null || program === undefined
      ? null
      : isRecord(program) && typeof program["name"] === "string"
        ? { name: program["name"] }
        : null;

  return {
    apiVersion,
    program: parsedProgram,
    ...(ir === undefined ? {} : { ir: parseModuleIr(ir) }),
    diagnostics: diagnostics.map(parseDiagnostic),
    metadata,
    artifacts,
  };
}

/**
 * Minimal JSON-RPC client for the native Rust compiler.
 *
 * The client runs one request per process. It never parses or transforms `.ns`
 * source itself; formatting and diagnostics come from the Rust core.
 */
export class CompilerClient {
  readonly binaryPath: string;

  private readonly cwd: string;
  private readonly timeoutMs: number | undefined;

  constructor(options: CompilerClientOptions = {}) {
    this.binaryPath = findCompilerBinary(options);
    this.cwd = resolve(options.cwd ?? process.cwd());
    this.timeoutMs = options.timeoutMs;
  }

  /** Compiles one source unit and returns typed IR for a backend. */
  compile(request: CompileRequest): CompileResponse {
    const payload = JSON.stringify({
      jsonrpc: "2.0",
      id: "astra-compile",
      method: "compile",
      params: {
        apiVersion: COMPILER_API_VERSION,
        source: request.source,
        target: request.target ?? "js",
        ...(request.mode === undefined ? {} : { mode: request.mode }),
        ...(request.integerPolicy === undefined
          ? {}
          : { integerPolicy: request.integerPolicy }),
      },
    });

    const result = spawnSync(this.binaryPath, [], {
      input: payload,
      cwd: this.cwd,
      encoding: "utf8",
      maxBuffer: 32 * 1024 * 1024,
      windowsHide: true,
      ...(this.timeoutMs === undefined ? {} : { timeout: this.timeoutMs }),
    });

    if (result.error !== undefined) {
      throw new CompilerClientError(
        "NS9004",
        `Unable to run compiler '${this.binaryPath}': ${result.error.message}`,
        { cause: result.error },
      );
    }

    if (result.status !== 0) {
      const detail = result.stderr.trim();
      throw new CompilerClientError(
        "NS9004",
        `Compiler exited with status ${result.status ?? "unknown"}${detail.length === 0 ? "" : `: ${detail}`}`,
      );
    }

    const output = result.stdout.trim();
    if (output.length === 0) {
      throw new CompilerClientError("NS9005", "Compiler returned an empty response.");
    }

    let response: unknown;
    try {
      response = JSON.parse(output);
    } catch (error) {
      throw new CompilerClientError("NS9005", "Compiler returned invalid JSON.", {
        cause: error,
      });
    }

    if (!isRecord(response) || response["jsonrpc"] !== "2.0") {
      throw new CompilerClientError("NS9005", "Compiler returned an invalid JSON-RPC response.");
    }

    if (response["error"] !== undefined && response["error"] !== null) {
      const rpcError = response["error"];
      const code =
        isRecord(rpcError) && typeof rpcError["code"] === "number"
          ? rpcError["code"]
          : -32603;
      const message =
        isRecord(rpcError) && typeof rpcError["message"] === "string"
          ? rpcError["message"]
          : "Unknown compiler error.";
      throw new CompilerClientError("NS9006", `Compiler request failed (${code}): ${message}`);
    }

    return parseCompileResponse(response["result"]);
  }

  /** Formats one source unit through the native compiler. */
  format(request: FormatRequest): FormatResponse {
    const payload = JSON.stringify({
      jsonrpc: "2.0",
      id: "astra-format",
      method: "format",
      params: {
        apiVersion: COMPILER_API_VERSION,
        source: request.source,
        target: request.target ?? "js",
        ...(request.mode === undefined ? {} : { mode: request.mode }),
      },
    });

    const result = spawnSync(this.binaryPath, [], {
      input: payload,
      cwd: this.cwd,
      encoding: "utf8",
      maxBuffer: 16 * 1024 * 1024,
      windowsHide: true,
      ...(this.timeoutMs === undefined ? {} : { timeout: this.timeoutMs }),
    });

    if (result.error !== undefined) {
      throw new CompilerClientError(
        "NS9004",
        `Unable to run compiler '${this.binaryPath}': ${result.error.message}`,
        { cause: result.error },
      );
    }

    if (result.status !== 0) {
      const detail = result.stderr.trim();
      throw new CompilerClientError(
        "NS9004",
        `Compiler exited with status ${result.status ?? "unknown"}${detail.length === 0 ? "" : `: ${detail}`}`,
      );
    }

    const output = result.stdout.trim();
    if (output.length === 0) {
      throw new CompilerClientError("NS9005", "Compiler returned an empty response.");
    }

    let response: unknown;
    try {
      response = JSON.parse(output);
    } catch (error) {
      throw new CompilerClientError("NS9005", "Compiler returned invalid JSON.", { cause: error });
    }

    if (!isRecord(response) || response["jsonrpc"] !== "2.0") {
      throw new CompilerClientError("NS9005", "Compiler returned an invalid JSON-RPC response.");
    }

    if (response["error"] !== undefined && response["error"] !== null) {
      const rpcError = response["error"];
      const code =
        isRecord(rpcError) && typeof rpcError["code"] === "number"
          ? rpcError["code"]
          : -32603;
      const message =
        isRecord(rpcError) && typeof rpcError["message"] === "string"
          ? rpcError["message"]
          : "Unknown compiler error.";
      throw new CompilerClientError("NS9006", `Compiler request failed (${code}): ${message}`);
    }

    return parseFormatResponse(response["result"]);
  }

  /** Convenience wrapper for formatting a path and source string. */
  formatSource(
    path: string,
    text: string,
    options: Pick<FormatRequest, "target" | "mode"> = {},
  ): FormatResponse {
    return this.format({ source: { path, text }, ...options });
  }
}

// ─── Project (multi-source) check ────────────────────────────────────────────

/** One source unit in a project check request. */
export interface ProjectSourceUnit {
  /** Canonical module key as used in `import` statements. */
  readonly key: string;
  /** File path shown in diagnostics. */
  readonly path: string;
  /** Source text. */
  readonly text: string;
}

/** One explainable reason contributing to a mode decision. */
export interface ModeReason {
  readonly kind: string;
  readonly mode: string;
  readonly detail: string;
  readonly capability?: string;
  readonly module?: string;
  readonly span?: DiagnosticSpan;
}

/** Parameters for `CompilerClient.checkProject`. */
export interface ProjectCheckRequest {
  /** Ordered list of source units. Dependencies must precede importers. */
  readonly sources: readonly ProjectSourceUnit[];
  readonly target?: string;
  readonly mode?: Mode;
}

/** Per-module result returned by `check_project`. */
export interface ModuleCheckResult {
  readonly key: string;
  readonly path: string;
  readonly diagnostics: readonly CompilerDiagnostic[];
  readonly ok: boolean;
}

/** Response returned by `CompilerClient.checkProject`. */
export interface ProjectCheckResponse {
  readonly apiVersion: string;
  readonly ok: boolean;
  readonly modules: readonly ModuleCheckResult[];
  readonly program: { readonly name: string } | null;
  readonly metadata: {
    readonly moduleOrder: readonly string[];
    readonly target: string;
    /** Explicit or inferred mode selected for the project check. */
    readonly mode: string;
    /** `explicit` when the request supplied a mode, otherwise `inferred`. */
    readonly modeSource?: "explicit" | "inferred";
    readonly requiredMode?: string;
    readonly inferredModes?: Readonly<Record<string, string>>;
    readonly modeReasons?: readonly ModeReason[];
    readonly modeExplanation?: string;
    [key: string]: unknown;
  };
}

function parseModuleCheckResult(value: unknown): ModuleCheckResult {
  if (!isRecord(value)) {
    throw new CompilerClientError("NS9005", "Compiler returned an invalid module result.");
  }
  const { key, path, ok } = value;
  const diagnosticsRaw = value["diagnostics"];
  if (
    typeof key !== "string" ||
    typeof path !== "string" ||
    typeof ok !== "boolean" ||
    !Array.isArray(diagnosticsRaw)
  ) {
    throw new CompilerClientError("NS9005", "Compiler returned an invalid module result.");
  }
  return {
    key,
    path,
    ok,
    diagnostics: diagnosticsRaw.map(parseDiagnostic),
  };
}

function parseProjectCheckResponse(value: unknown): ProjectCheckResponse {
  if (!isRecord(value)) {
    throw new CompilerClientError("NS9005", "Compiler returned an invalid project check result.");
  }
  const { apiVersion, ok, modules, program, metadata } = value;
  if (
    typeof apiVersion !== "string" ||
    apiVersion !== COMPILER_API_VERSION ||
    typeof ok !== "boolean" ||
    !Array.isArray(modules)
  ) {
    throw new CompilerClientError(
      "NS9005",
      `Compiler returned an incompatible project check result; expected API ${COMPILER_API_VERSION}.`,
    );
  }

  const parsedProgram =
    program === null || program === undefined
      ? null
      : isRecord(program) && typeof program["name"] === "string"
        ? { name: program["name"] }
        : null;

  const parsedMetadata = isRecord(metadata)
    ? (metadata as { moduleOrder: string[]; target: string; mode: string })
    : { moduleOrder: [], target: "js", mode: "core" };

  return {
    apiVersion,
    ok,
    modules: modules.map(parseModuleCheckResult),
    program: parsedProgram,
    metadata: parsedMetadata,
  };
}

// Extend CompilerClient with checkProject ─────────────────────────────────────

declare module "./index.js" {}

// We extend the class by augmenting the prototype so the existing build output
// is not disturbed. TypeScript sees the method through the interface merge below.

CompilerClient.prototype.checkProject = function (
  this: CompilerClient,
  request: ProjectCheckRequest,
): ProjectCheckResponse {
  const payload = JSON.stringify({
    jsonrpc: "2.0",
    id: "astra-check-project",
    method: "check_project",
    params: {
      apiVersion: COMPILER_API_VERSION,
      sources: request.sources,
      target: request.target ?? "js",
      ...(request.mode === undefined ? {} : { mode: request.mode }),
    },
  });

  const result = spawnSync(this.binaryPath, [], {
    input: payload,
    cwd: (this as unknown as { cwd: string }).cwd,
    encoding: "utf8",
    maxBuffer: 16 * 1024 * 1024,
    windowsHide: true,
  });

  if (result.error !== undefined) {
    throw new CompilerClientError(
      "NS9004",
      `Unable to run compiler '${this.binaryPath}': ${result.error.message}`,
      { cause: result.error },
    );
  }
  if (result.status !== 0) {
    const detail = result.stderr.trim();
    throw new CompilerClientError(
      "NS9004",
      `Compiler exited with status ${result.status ?? "unknown"}${detail.length === 0 ? "" : `: ${detail}`}`,
    );
  }

  const output = result.stdout.trim();
  if (output.length === 0) {
    throw new CompilerClientError("NS9005", "Compiler returned an empty response.");
  }

  let response: unknown;
  try {
    response = JSON.parse(output);
  } catch (error) {
    throw new CompilerClientError("NS9005", "Compiler returned invalid JSON.", { cause: error });
  }

  if (!isRecord(response) || response["jsonrpc"] !== "2.0") {
    throw new CompilerClientError("NS9005", "Compiler returned an invalid JSON-RPC response.");
  }

  if (response["error"] !== undefined && response["error"] !== null) {
    const rpcError = response["error"];
    const code =
      isRecord(rpcError) && typeof rpcError["code"] === "number" ? rpcError["code"] : -32603;
    const message =
      isRecord(rpcError) && typeof rpcError["message"] === "string"
        ? rpcError["message"]
        : "Unknown compiler error.";
    throw new CompilerClientError("NS9006", `Compiler request failed (${code}): ${message}`);
  }

  return parseProjectCheckResponse(response["result"]);
};

/** Augment `CompilerClient` to include `checkProject`. */
export interface CompilerClient {
  /**
   * Checks a multi-module project through the Rust `check_project` method.
   *
   * `sources` must be in dependency order: each module before any module that
   * imports it. The entry-point module should appear last.
   */
  checkProject(request: ProjectCheckRequest): ProjectCheckResponse;
}
