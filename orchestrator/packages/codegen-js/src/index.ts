//! JavaScript ESM backend for Neo Astra.
//!
//! The backend consumes the typed AST/HIR returned by the Rust compiler and
//! emits readable, framework-free ES modules. It never parses `.ns` source.

export {
  CodegenRequestError,
  generateJavaScript,
  generateModule,
  type CodegenDiagnostic,
  type CodegenRequest,
  type CodegenResult,
  type TypeScriptTranspileOutcome,
  type TypeScriptTranspiler,
} from "./generate.js";
