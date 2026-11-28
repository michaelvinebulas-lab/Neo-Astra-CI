/**
 * Stable message shapes between the playground UI and the compiler Worker.
 *
 * The Worker owns a `CompilerBridge` instance. The UI never talks to Wasm
 * directly, so every request can be timed out and the Worker can be terminated
 * without leaving a compiler instance on the main thread.
 */

/** Protocol version for the Worker transport. Independent of the compiler API. */
export const PLAYGROUND_PROTOCOL_VERSION = 1;

/** Compiler API version this host was written against. */
export const COMPILER_API_VERSION = "1.0" as const;

/** Language editions understood by the playground. */
export type Edition = "0.1" | "0.2";

/** Capability modes understood by the compiler boundary. */
export type Mode = "core" | "app" | "web" | "system";

/** Integer arithmetic policy from the build manifest. */
export type IntegerPolicy = "wrapping" | "safe-checked" | "arbitrary";

/** Every method the compiler Worker is allowed to dispatch. */
export type CompilerMethod =
  | "initialize"
  | "check"
  | "check_project"
  | "compile"
  | "compile_project"
  | "ast"
  | "format"
  | "canonical"
  | "explain"
  | "migrate"
  | "shutdown";

/** Byte span reported by the Rust compiler. */
export interface DiagnosticSpan {
  readonly start: number;
  readonly end: number;
}

/** Structured diagnostic shape shared by all compiler panels. */
export interface StructuredDiagnostic {
  readonly code: string;
  readonly severity: "error" | "warning" | "note" | "help";
  readonly message: string;
  readonly span: DiagnosticSpan | null;
  readonly hint: string | null;
  readonly related?: readonly {
    readonly file: string;
    readonly span: DiagnosticSpan;
    readonly message: string;
  }[];
}

/** Parameters for the `canonical` JSON-RPC command. */
export interface CanonicalParams {
  readonly apiVersion: typeof COMPILER_API_VERSION;
  readonly source: {
    readonly path: string;
    readonly text: string;
  };
  readonly edition?: Edition;
  readonly target?: string;
}

/** Result of the `canonical` JSON-RPC command. */
export interface CanonicalResult {
  readonly apiVersion: string;
  readonly ok: boolean;
  readonly diagnostics: readonly StructuredDiagnostic[];
}

/** Parameters for a single-source compiler request. */
export interface SourceRequestParams {
  readonly apiVersion: typeof COMPILER_API_VERSION;
  readonly source: {
    readonly path: string;
    readonly text: string;
  };
  readonly target?: string;
  readonly mode?: Mode;
  readonly edition?: Edition;
  readonly integerPolicy?: IntegerPolicy;
}

/** One ordered source unit in a project request. */
export interface ProjectSource {
  readonly key: string;
  readonly path: string;
  readonly text: string;
}

/** Parameters for `check_project`. */
export interface ProjectCheckParams {
  readonly apiVersion: typeof COMPILER_API_VERSION;
  readonly sources: readonly ProjectSource[];
  readonly target: string;
  readonly mode?: Mode;
  readonly edition?: Edition;
  readonly integerPolicy?: IntegerPolicy;
}

/** A request from the UI to the Worker. */
export interface WorkerRequest<Params = unknown> {
  readonly id: number;
  readonly method: CompilerMethod;
  readonly params: Params;
}

/** Successful Worker response. */
export interface WorkerSuccess<Result = unknown> {
  readonly id: number;
  readonly ok: true;
  readonly result: Result;
}

/** Worker-level failure. Compiler diagnostics arrive as successful results. */
export interface WorkerFailure {
  readonly id: number;
  readonly ok: false;
  readonly error: {
    readonly code: string;
    readonly message: string;
    readonly data?: unknown;
  };
}

/** Response from the Worker to the UI. */
export type WorkerResponse<Result = unknown> = WorkerSuccess<Result> | WorkerFailure;

/** Worker transport error codes. They are not compiler diagnostics. */
export type WorkerErrorCode =
  | "PGW0001" // unsupported protocol version
  | "PGW0002" // compiler request timed out
  | "PGW0003" // unknown method
  | "PGW0004" // bridge failure
  | "PGW0005"; // malformed message

/** Guard for narrowing a postMessage payload to a Worker response. */
export function isWorkerResponse(value: unknown): value is WorkerResponse {
  if (typeof value !== "object" || value === null) return false;
  const record = value as { id?: unknown; ok?: unknown };
  return typeof record.id === "number" && (record.ok === true || record.ok === false);
}
