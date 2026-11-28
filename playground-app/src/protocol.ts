export const PLAYGROUND_PROTOCOL_VERSION = 1;

export const COMPILER_API_VERSION = "1.0" as const;

export type Edition = "0.1" | "0.2";

export type Mode = "core" | "app" | "web" | "system";

export type IntegerPolicy = "wrapping" | "safe-checked" | "arbitrary";

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

export interface DiagnosticSpan {
  readonly start: number;
  readonly end: number;
}

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

export interface CanonicalParams {
  readonly apiVersion: typeof COMPILER_API_VERSION;
  readonly source: {
    readonly path: string;
    readonly text: string;
  };
  readonly edition?: Edition;
  readonly target?: string;
}

export interface CanonicalResult {
  readonly apiVersion: string;
  readonly ok: boolean;
  readonly diagnostics: readonly StructuredDiagnostic[];
}

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

export interface ProjectSource {
  readonly key: string;
  readonly path: string;
  readonly text: string;
}

export interface ProjectCheckParams {
  readonly apiVersion: typeof COMPILER_API_VERSION;
  readonly sources: readonly ProjectSource[];
  readonly target: string;
  readonly mode?: Mode;
  readonly edition?: Edition;
  readonly integerPolicy?: IntegerPolicy;
}

/** One module result inside `check_project`. */
export interface ModuleCheckResult {
  readonly key: string;
  readonly path: string;
  readonly ok: boolean;
  readonly diagnostics: readonly StructuredDiagnostic[];
  readonly ir?: unknown;
}

/** Result of `check_project`. */
export interface ProjectCheckResult {
  readonly apiVersion: string;
  readonly ok: boolean;
  readonly program: { readonly name: string } | null;
  readonly metadata: Readonly<Record<string, unknown>>;
  readonly modules: readonly ModuleCheckResult[];
}

/** Result of `format`. */
export interface FormatResult {
  readonly apiVersion: string;
  readonly source: string | null;
  readonly diagnostics: readonly StructuredDiagnostic[];
}

/** Result of `ast`. */
export interface AstResult {
  readonly apiVersion: string;
  readonly ok: boolean;
  readonly ast?: unknown;
  readonly diagnostics: readonly StructuredDiagnostic[];
}

/** Result of `explain`. */
export interface ExplainResult {
  readonly apiVersion: string;
  readonly ok: boolean;
  readonly inference?: unknown;
  readonly explanation?: string;
  readonly diagnostics: readonly StructuredDiagnostic[];
}

/** Parameters for the `migrate` JSON-RPC command. */
export interface MigrateParams {
  readonly apiVersion: typeof COMPILER_API_VERSION;
  readonly source: {
    readonly path: string;
    readonly text: string;
  };
  readonly edition?: Edition;
  readonly changeEdition?: boolean;
}

/** Result of the `migrate` JSON-RPC command. */
export interface MigrateResult {
  readonly apiVersion: string;
  readonly ok: boolean;
  readonly changed: boolean;
  readonly source: string;
  readonly diagnostics: readonly StructuredDiagnostic[];
}

/** Every result type the Worker can return. */
export type WorkerResult =
  | ProjectCheckResult
  | FormatResult
  | AstResult
  | ExplainResult
  | CanonicalResult
  | MigrateResult
  | null;

export interface WorkerRequest<Params = unknown> {
  readonly id: number;
  readonly method: CompilerMethod;
  readonly params: Params;
}

export interface WorkerSuccess<Result = unknown> {
  readonly id: number;
  readonly ok: true;
  readonly result: Result;
}

export interface WorkerFailure {
  readonly id: number;
  readonly ok: false;
  readonly error: {
    readonly code: string;
    readonly message: string;
    readonly data?: unknown;
  };
}

export type WorkerResponse<Result = unknown> = WorkerSuccess<Result> | WorkerFailure;

export type WorkerErrorCode =
  | "PGW0001"
  | "PGW0002"
  | "PGW0003"
  | "PGW0004"
  | "PGW0005";

export function isWorkerResponse(value: unknown): value is WorkerResponse {
  if (typeof value !== "object" || value === null) return false;
  const record = value as { id?: unknown; ok?: unknown };
  return typeof record.id === "number" && (record.ok === true || record.ok === false);
}
