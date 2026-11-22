//! TypeScript backend for Neo Astra.
//!
//! The backend consumes the same serialized typed module as `codegen-js` and
//! runs it through the same shared emission pipeline (`generateModule`), with
//! `backend: "ts"` adding explicit type annotations, body-level type
//! declarations, and typed runtime helpers. It never parses `.ns` source and
//! never forks runtime semantics from the JavaScript backend.

import { generateModule } from "@neo-astra/codegen-js";
import type {
  CodegenDiagnostic,
  CodegenRequest,
  CodegenResult,
} from "@neo-astra/codegen-js";

/** Input accepted by `generateTypeScript`. Same typed module as `codegen-js`. */
export interface TypeScriptCodegenRequest extends Omit<CodegenRequest, "backend"> {}

export type { CodegenDiagnostic, CodegenRequest, CodegenResult };
export { CodegenRequestError } from "@neo-astra/codegen-js";

/**
 * Generates strict TypeScript ESM for one typed module.
 *
 * The output is the JavaScript backend's emission with explicit types layered
 * on top: typed function signatures, annotated bindings, `interface`/`type`
 * declarations for local nominal types, and `tsc --strict`-clean helpers —
 * identical runtime semantics, one pipeline, one source of truth.
 */
export function generateTypeScript(
  request: TypeScriptCodegenRequest,
): CodegenResult {
  return generateModule({ ...request, backend: "ts" });
}
