//! JavaScript ESM backend for Neo Astra.
//!
//! The backend consumes the typed AST/HIR returned by the Rust compiler and
//! emits readable, framework-free ES modules. It never parses `.ns` source.

export {
  CodegenRequestError,
  generateJavaScript,
  type CodegenDiagnostic,
  type CodegenRequest,
  type CodegenResult,
} from "./generate.js";
