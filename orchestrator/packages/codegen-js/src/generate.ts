//! Readable JavaScript ESM generation from a typed Neo Astra module.
//!
//! The backend consumes the checked AST and resolved HIR exported by the Rust
//! compiler. It emits framework-free ESM and rejects constructs whose semantics
//! are not implemented yet rather than silently changing them.

import {
  type AstModule,
  type AstType,
  type Block,
  type EnumDecl,
  type Expr,
  type HirItem,
  type HirModule,
  type Literal,
  type MatchArm,
  type ModuleIr,
  type Pattern,
  type SemanticType,
  type Span,
  type Stmt,
  type StructDecl,
  type TypePath,
  isRecord,
  payloadOf,
  structPayload,
  typeTag,
  variantOf,
} from "./ir.js";

/** One JavaScript backend diagnostic. */
export interface CodegenDiagnostic {
  readonly code: string;
  readonly message: string;
  readonly span?: Span;
}

/** Input accepted by `generateJavaScript`. */
export interface CodegenRequest {
  readonly moduleName: string;
  readonly ir: unknown;
  readonly sourceText?: string;
}

/** Output produced by `generateJavaScript`. */
export interface CodegenResult {
  readonly code: string;
  readonly diagnostics: readonly CodegenDiagnostic[];
  readonly sourceMap?: string;
}

/** The requested typed module is malformed. */
export class CodegenRequestError extends Error {
  readonly code = "NS8002";

  constructor(message: string) {
    super(message);
    this.name = "CodegenRequestError";
  }
}

const JS_RESERVED = new Set([
  "await",
  "break",
  "case",
  "catch",
  "class",
  "const",
  "continue",
  "debugger",
  "default",
  "delete",
  "do",
  "else",
  "enum",
  "export",
  "extends",
  "false",
  "finally",
  "for",
  "function",
  "if",
  "implements",
  "import",
  "in",
  "instanceof",
  "interface",
  "let",
  "new",
  "null",
  "package",
  "private",
  "protected",
  "public",
  "return",
  "static",
  "super",
  "switch",
  "this",
  "throw",
  "true",
  "try",
  "typeof",
  "undefined",
  "var",
  "void",
  "while",
  "with",
  "yield",
]);

const IDENTIFIER = /^[A-Za-z_$][A-Za-z0-9_$]*$/;

const HELPER_ORDER = [
  "panic",
  "abort",
  "failure",
  "fail",
  "taskScope",
  "spawn",
  "taskJoin",
  "taskCancel",
  "renderFloat",
  "render",
  "renderDebug",
  "float",
  "write",
  "print",
  "println",
  "dbg",
  "assert",
  "utf8Len",
  "len",
  "strAt",
  "index",
  "getArray",
  "tuple",
  "tupleAt",
  "map",
  "mapGet",
  "mapInsert",
  "mapKeys",
  "eq",
  "none",
  "some",
  "ok",
  "err",
  "unwrapOption",
  "unwrapResult",
  "resultError",
  "parseInt",
  "div",
  "rem",
  "divBig",
  "remBig",
  "i8",
  "i16",
  "i32",
  "u8",
  "u16",
  "u32",
  "i64",
  "u64",
  "checked_i8",
  "checked_i16",
  "checked_i32",
  "checked_u8",
  "checked_u16",
  "checked_u32",
  "checked_i64",
  "checked_u64",
  "checkedInteger",
] as const;

type HelperName = (typeof HELPER_ORDER)[number];

const HELPER_DEPENDENCIES: Partial<Record<HelperName, readonly HelperName[]>> = {
  fail: ["failure"],
  spawn: ["taskScope"],
  taskJoin: ["failure"],
  render: ["renderFloat"],
  print: ["write", "render"],
  println: ["write", "render"],
  dbg: ["render"],
  assert: ["panic"],
  len: ["panic", "utf8Len"],
  strAt: ["panic"],
  index: ["panic"],
  mapGet: ["eq", "none"],
  mapInsert: ["eq", "none", "some"],
  mapKeys: [],
  getArray: ["none"],
  parseInt: ["ok", "err"],
  div: ["panic"],
  rem: ["panic"],
  divBig: ["panic"],
  remBig: ["panic"],
  checked_i8: ["panic"],
  checked_i16: ["panic"],
  checked_i32: ["panic"],
  checked_u8: ["panic"],
  checked_u16: ["panic"],
  checked_u32: ["panic"],
  checked_i64: ["panic"],
  checked_u64: ["panic"],
  checkedInteger: ["ok", "err", "panic"],
  unwrapResult: ["panic"],
  resultError: ["panic"],
  unwrapOption: ["panic"],
};

const HELPERS: Record<HelperName, readonly string[]> = {
  panic: [
    "function $panic(message) {",
    '  throw new Error("panic: " + message);',
    "}",
  ],
  abort: [
    "function $abort(message) {",
    '  throw new Error("abort: " + message);',
    "}",
  ],
  failure: [
    "function $failure(error) {",
    "  return { $failure: true, error };",
    "}",
  ],
  fail: [
    "function $fail(error) {",
    "  throw $failure(error);",
    "}",
  ],
  taskScope: [
    "const $taskScopes = [];",
    "function $enterTaskScope() {",
    "  const scope = { tasks: [] };",
    "  $taskScopes.push(scope);",
    "  return scope;",
    "}",
    "function $exitTaskScope(scope) {",
    "  $taskScopes.pop();",
    "  for (const task of scope.tasks) {",
    "    if (!task.joined && task.outcome && task.outcome.panic !== undefined) throw task.outcome.panic;",
    "  }",
    "}",
  ],
  spawn: [
    "function $spawn(body) {",
    "  const task = { $task: true, cancelled: false, joined: false, outcome: undefined };",
    "  try {",
    "    task.outcome = { ok: true, value: body() };",
    "  } catch (error) {",
    "    if (error && error.$failure) task.outcome = { ok: false, error: error.error };",
    "    else task.outcome = { panic: error };",
    "  }",
    "  if ($taskScopes.length > 0) $taskScopes[$taskScopes.length - 1].tasks.push(task);",
    "  return task;",
    "}",
  ],
  taskJoin: [
    "function $taskJoin(task) {",
    "  task.joined = true;",
    "  if (task.cancelled) {",
    '    return $failure({ $type: "TaskError", 0: "task was cancelled" });',
    "  }",
    "  if (task.outcome.panic !== undefined) throw task.outcome.panic;",
    "  if (!task.outcome.ok) return $failure(task.outcome.error);",
    "  return task.outcome.value;",
    "}",
  ],
  taskCancel: [
    "function $taskCancel(task) {",
    "  task.cancelled = true;",
    "  return undefined;",
    "}",
  ],
  renderFloat: [
    "function $renderFloat(value) {",
    '  if (Number.isNaN(value)) return "NaN";',
    '  if (value === Infinity) return "Infinity";',
    '  if (value === -Infinity) return "-Infinity";',
    "  if (Number.isInteger(value) && Math.abs(value) < 1e16) return value.toFixed(1);",
    "  return String(value);",
    "}",
  ],
  render: [
    "function $render(value, debug = false) {",
    '  if (value === undefined) return "()";',
    '  if (value === null) return "None";',
    '  if (typeof value === "bigint") return value.toString();',
    '  if (typeof value === "string") return debug ? JSON.stringify(value) : value;',
    '  if (typeof value === "number" || typeof value === "boolean") return String(value);',
    '  if (value instanceof Uint8Array) {',
    '    let output = "b\\"";',
    "    for (const byte of value) {",
    '      if (byte === 92) output += "\\\\\\\\";',
    '      else if (byte === 34) output += "\\\\\\"";',
    '      else if (byte === 10) output += "\\\\n";',
    '      else if (byte === 13) output += "\\\\r";',
    '      else if (byte === 9) output += "\\\\t";',
    '      else if (byte >= 32 && byte <= 126) output += String.fromCharCode(byte);',
    '      else output += "\\\\u{" + byte.toString(16).padStart(2, "0") + "}";',
    "    }",
    '    return output + "\\"";',
    "  }",
    "  if (Array.isArray(value)) {",
    '    return "[" + value.map((item) => $render(item, debug)).join(", ") + "]";',
    "  }",
    "  if (value.$tuple) {",
    '    const items = value.$tuple.map((item) => $render(item, debug));',
    '    return "(" + items.join(", ") + (items.length === 1 ? "," : "") + ")";',
    "  }",
    "  if (value.$map) {",
    '    return "{" + value.$map.map(([key, item]) => $render(key, debug) + ": " + $render(item, debug)).join(", ") + "}";',
    "  }",
    "  if (typeof value.$float === \"number\") return $renderFloat(value.$float);",
    "  if (typeof value.$option === \"boolean\") {",
    '    return value.$option ? "Some(" + $render(value.value, debug) + ")" : "None";',
    "  }",
    "  if (value.$result) {",
    '    return value.ok ? "Ok(" + $render(value.value, debug) + ")" : "Err(" + $render(value.error, debug) + ")";',
    "  }",
    "  if (value.$tag) {",
    '    let output = value.$enum + "." + value.$tag;',
    '    const fields = Object.keys(value).filter((key) => /^[0-9]+$/.test(key)).sort((a, b) => Number(a) - Number(b));',
    "    if (fields.length > 0) {",
    '      output += "(" + fields.map((key) => $render(value[key], debug)).join(", ") + ")";',
    "    }",
    "    return output;",
    "  }",
    "  if (value.$type) {",
    "    const numeric = Object.keys(value).filter((key) => /^[0-9]+$/.test(key));",
    "    if (numeric.length > 0) {",
    '      const fields = numeric.sort((a, b) => Number(a) - Number(b)).map((key) => $render(value[key], debug));',
    '      return value.$type + "(" + fields.join(", ") + ")";',
    "    }",
    '    const fields = Object.keys(value).filter((key) => key !== "$type").map((key) => key + ": " + $render(value[key], debug));',
    '    return value.$type + " { " + fields.join(", ") + " }";',
    "  }",
    "  return String(value);",
    "}",
  ],
  renderDebug: [
    "function $renderDebug(value) {",
    "  return $render(value, true);",
    "}",
  ],
  float: [
    "function $float(value) {",
    "  return {",
    "    $float: Number(value),",
    "    valueOf() { return this.$float; },",
    "  };",
    "}",
  ],
  write: [
    "function $write(text) {",
    '  if (typeof process !== "undefined" && process.stdout && typeof process.stdout.write === "function") {',
    "    process.stdout.write(text);",
    "    return;",
    "  }",
    '  if (typeof console !== "undefined") console.log(text);',
    "}",
  ],
  print: [
    "function $print(value) {",
    "  $write($render(value, false));",
    "}",
  ],
  println: [
    "function $println(value) {",
    '  $write($render(value, false) + "\\n");',
    "}",
  ],
  dbg: [
    "function $dbg(value) {",
    "  const line = $render(value, true);",
    '  if (typeof process !== "undefined" && process.stderr && typeof process.stderr.write === "function") {',
    '    process.stderr.write(line + "\\n");',
    "    return;",
    "  }",
    '  if (typeof console !== "undefined") console.error(line);',
    "}",
  ],
  assert: [
    "function $assert(condition) {",
    '  if (!condition) $panic("assertion failed");',
    "}",
  ],
  utf8Len: [
    "function $utf8Len(value) {",
    "  return new TextEncoder().encode(value).length;",
    "}",
  ],
  len: [
    "function $len(value) {",
    '  if (typeof value === "string") return $utf8Len(value);',
    "  if (Array.isArray(value) || value instanceof Uint8Array) return value.length;",
    "  if (value && value.$map) return value.$map.length;",
    '  $panic("len() is not implemented for this value");',
    "}",
  ],
  strAt: [
    "function $strAt(value, index) {",
    "  const characters = Array.from(value);",
    '  if (!Number.isInteger(index) || index < 0 || index >= characters.length) $panic("index out of bounds");',
    "  return characters[index];",
    "}",
  ],
  index: [
    "function $index(value, index) {",
    '  if (!Number.isInteger(index) || index < 0 || index >= value.length) $panic("index out of bounds");',
    "  return value[index];",
    "}",
  ],
  getArray: [
    "function $getArray(value, index) {",
    '  if (!Number.isInteger(index) || index < 0 || index >= value.length) return $none;',
    "  return $some(value[index]);",
    "}",
  ],
  tuple: [
    "function $tuple(values) {",
    "  return { $tuple: values };",
    "}",
  ],
  tupleAt: [
    "function $tupleAt(value, index) {",
    '  if (!Number.isInteger(index) || index < 0 || index >= value.$tuple.length) $panic("index out of bounds");',
    "  return value.$tuple[index];",
    "}",
  ],
  map: [
    "function $map(entries) {",
    "  const values = [];",
    "  for (const [key, value] of entries) {",
    "    const existing = values.findIndex(([candidate]) => $eq(candidate, key));",
    "    if (existing >= 0) values[existing] = [key, value];",
    "    else values.push([key, value]);",
    "  }",
    "  return { $map: values };",
    "}",
  ],
  mapGet: [
    "function $mapGet(map, key) {",
    "  const entry = map.$map.find(([candidate]) => $eq(candidate, key));",
    "  return entry === undefined ? $none : $some(entry[1]);",
    "}",
  ],
  mapInsert: [
    "function $mapInsert(map, key, value) {",
    "  const index = map.$map.findIndex(([candidate]) => $eq(candidate, key));",
    "  if (index < 0) {",
    "    map.$map.push([key, value]);",
    "    return $none;",
    "  }",
    "  const previous = map.$map[index][1];",
    "  map.$map[index] = [key, value];",
    "  return $some(previous);",
    "}",
  ],
  mapKeys: [
    "function $mapKeys(map) {",
    "  return map.$map.map(([key]) => key);",
    "}",
  ],
  eq: [
    "function $eq(left, right) {",
    "  if (left === right) return true;",
    '  if (typeof left !== "object" || typeof right !== "object" || left === null || right === null) return false;',
    "  if (typeof left.$float === \"number\" || typeof right.$float === \"number\") {",
    "    return typeof left.$float === \"number\" && typeof right.$float === \"number\" && left.$float === right.$float;",
    "  }",
    "  if (left instanceof Uint8Array || right instanceof Uint8Array) {",
    "    if (!(left instanceof Uint8Array) || !(right instanceof Uint8Array)) return false;",
    "    if (left.length !== right.length) return false;",
    "    for (let index = 0; index < left.length; index += 1) if (left[index] !== right[index]) return false;",
    "    return true;",
    "  }",
    "  if (Array.isArray(left) || Array.isArray(right)) {",
    "    if (!Array.isArray(left) || !Array.isArray(right)) return false;",
    "    return left.length === right.length && left.every((item, index) => $eq(item, right[index]));",
    "  }",
    "  if (left.$tuple || right.$tuple) {",
    "    return Boolean(left.$tuple && right.$tuple) && $eq(left.$tuple, right.$tuple);",
    "  }",
    "  if (left.$map || right.$map) {",
    "    if (!left.$map || !right.$map || left.$map.length !== right.$map.length) return false;",
    "    return left.$map.every(([key, value]) => {",
    "      const entry = right.$map.find(([candidate]) => $eq(candidate, key));",
    "      return entry !== undefined && $eq(value, entry[1]);",
    "    });",
    "  }",
    "  if (typeof left.$option === \"boolean\" || typeof right.$option === \"boolean\") {",
    "    return typeof left.$option === \"boolean\" && typeof right.$option === \"boolean\" &&",
    "      left.$option === right.$option && (left.$option === false || $eq(left.value, right.value));",
    "  }",
    "  if (left.$result || right.$result) {",
    "    return Boolean(left.$result && right.$result) && left.ok === right.ok &&",
    "      (left.ok ? $eq(left.value, right.value) : $eq(left.error, right.error));",
    "  }",
    "  if (left.$tag || right.$tag) {",
    "    if (!left.$tag || !right.$tag || left.$enum !== right.$enum || left.$tag !== right.$tag) return false;",
    "    const keys = Object.keys(left).filter((key) => /^[0-9]+$/.test(key));",
    "    if (keys.length !== Object.keys(right).filter((key) => /^[0-9]+$/.test(key)).length) return false;",
    "    return keys.every((key) => $eq(left[key], right[key]));",
    "  }",
    "  if (left.$type || right.$type) {",
    "    if (!left.$type || !right.$type || left.$type !== right.$type) return false;",
    "    const keys = Object.keys(left).filter((key) => key !== \"$type\");",
    "    if (keys.length !== Object.keys(right).filter((key) => key !== \"$type\").length) return false;",
    "    return keys.every((key) => Object.prototype.hasOwnProperty.call(right, key) && $eq(left[key], right[key]));",
    "  }",
    "  return false;",
    "}",
  ],
  none: ["const $none = Object.freeze({ $option: false });"],
  some: [
    "function $some(value) {",
    "  return { $option: true, value };",
    "}",
  ],
  ok: [
    "function $ok(value) {",
    "  return { $result: true, ok: true, value };",
    "}",
  ],
  err: [
    "function $err(error) {",
    "  return { $result: true, ok: false, error };",
    "}",
  ],
  unwrapOption: [
    "function $unwrapOption(value) {",
    '  if (value.$option === false) $panic("called unwrap() on None");',
    "  return value.value;",
    "}",
  ],
  unwrapResult: [
    "function $unwrapResult(value) {",
    '  if (!value.ok) $panic("called unwrap() on Err(" + $render(value.error) + ")");',
    "  return value.value;",
    "}",
  ],
  resultError: [
    "function $resultError(value) {",
    '  if (value.ok) $panic("called error() on Ok(" + $render(value.value) + ")");',
    "  return value.error;",
    "}",
  ],
  parseInt: [
    "function $parseInt(value) {",
    '  if (!/^[+-]?[0-9]+$/.test(value)) return $err({ $type: "ParseIntError", 0: value });',
    "  const parsed = Number(value);",
    "  if (!Number.isInteger(parsed) || parsed < -2147483648 || parsed > 2147483647) {",
    '    return $err({ $type: "ParseIntError", 0: value });',
    "  }",
    "  return $ok(parsed);",
    "}",
  ],
  div: [
    "function $div(left, right) {",
    '  if (right === 0) $panic("division by zero");',
    "  return Math.trunc(left / right);",
    "}",
  ],
  rem: [
    "function $rem(left, right) {",
    '  if (right === 0) $panic("division by zero");',
    "  return left % right;",
    "}",
  ],
  divBig: [
    "function $divBig(left, right) {",
    "  if (right === 0n) $panic(\"division by zero\");",
    "  return left / right;",
    "}",
  ],
  remBig: [
    "function $remBig(left, right) {",
    "  if (right === 0n) $panic(\"division by zero\");",
    "  return left % right;",
    "}",
  ],
  i8: ["function $i8(value) {", "  return (value << 24) >> 24;", "}"],
  i16: ["function $i16(value) {", "  return (value << 16) >> 16;", "}"],
  i32: ["function $i32(value) {", "  return value | 0;", "}"],
  u8: ["function $u8(value) {", "  return value & 0xff;", "}"],
  u16: ["function $u16(value) {", "  return value & 0xffff;", "}"],
  u32: ["function $u32(value) {", "  return value >>> 0;", "}"],
  i64: ["function $i64(value) {", "  return BigInt.asIntN(64, BigInt(value));", "}"],
  u64: ["function $u64(value) {", "  return BigInt.asUintN(64, BigInt(value));", "}"],
  checked_i8: [
    "function $checked_i8(value) {",
    '  if (value < -128 || value > 127) $panic("integer overflow");',
    "  return value;",
    "}",
  ],
  checked_i16: [
    "function $checked_i16(value) {",
    '  if (value < -32768 || value > 32767) $panic("integer overflow");',
    "  return value;",
    "}",
  ],
  checked_i32: [
    "function $checked_i32(value) {",
    '  if (value < -2147483648 || value > 2147483647) $panic("integer overflow");',
    "  return value;",
    "}",
  ],
  checked_u8: [
    "function $checked_u8(value) {",
    '  if (value < 0 || value > 255) $panic("integer overflow");',
    "  return value;",
    "}",
  ],
  checked_u16: [
    "function $checked_u16(value) {",
    '  if (value < 0 || value > 65535) $panic("integer overflow");',
    "  return value;",
    "}",
  ],
  checked_u32: [
    "function $checked_u32(value) {",
    '  if (value < 0 || value > 4294967295) $panic("integer overflow");',
    "  return value;",
    "}",
  ],
  checked_i64: [
    "function $checked_i64(value) {",
    '  if (value < -9223372036854775808n || value > 9223372036854775807n) $panic("integer overflow");',
    "  return value;",
    "}",
  ],
  checked_u64: [
    "function $checked_u64(value) {",
    '  if (value < 0n || value > 18446744073709551615n) $panic("integer overflow");',
    "  return value;",
    "}",
  ],
  checkedInteger: [
    "function $checkedInteger(tag, operation, left, right) {",
    "  const bounds = {",
    "    i8: [-128, 127],",
    "    i16: [-32768, 32767],",
    "    i32: [-2147483648, 2147483647],",
    "    i64: [-9223372036854775808n, 9223372036854775807n],",
    "    u8: [0, 255],",
    "    u16: [0, 65535],",
    "    u32: [0, 4294967295],",
    "    u64: [0n, 18446744073709551615n],",
    "  };",
    "  const [min, max] = bounds[tag];",
    "  let value;",
    "  if (operation === \"div\") {",
    "    if (right === 0 || right === 0n) $panic(\"division by zero\");",
    '    value = typeof left === "bigint" ? left / right : Math.trunc(left / right);',
    '  } else if (operation === "add") {',
    "    value = left + right;",
    '  } else if (operation === "sub") {',
    "    value = left - right;",
    '  } else if (operation === "mul") {',
    "    value = left * right;",
    "  } else {",
    '    $panic("unsupported checked integer operation");',
    "  }",
    "  if (value < min || value > max) {",
    '    return $err({ $type: "OverflowError", 0: "integer overflow in \'checked_" + operation + "\'" });',
    "  }",
    "  return $ok(value);",
    "}",
  ],
};

const INTEGER_TAGS = new Set(["I8", "I16", "I32", "I64", "U8", "U16", "U32", "U64"]);
const BIG_INTEGER_TAGS = new Set(["I64", "U64"]);

class JavaScriptGenerator {
  private readonly ir: ModuleIr;
  private readonly ast: AstModule;
  private readonly hir: HirModule;
  private readonly sourceText: string | undefined;
  private readonly annotations = new Map<string, SemanticType>();
  private readonly helpers = new Set<HelperName>();
  private readonly diagnostics: CodegenDiagnostic[] = [];
  private readonly reported = new Set<string>();
  private tempIndex = 0;
  private defaultExpression: Expr | undefined;
  private currentSuccessWrap = false;

  constructor(ir: ModuleIr, sourceText?: string) {
    this.ir = ir;
    this.sourceText = sourceText;
    if (!isRecord(ir.ast) || !isRecord(ir.hir)) {
      throw new CodegenRequestError("Typed module AST and HIR must be objects.");
    }
    if (!Array.isArray(ir.ast["items"]) || !Array.isArray(ir.hir["items"])) {
      throw new CodegenRequestError("Typed module AST and HIR must contain item arrays.");
    }
    this.ast = ir.ast as unknown as AstModule;
    this.hir = ir.hir as unknown as HirModule;
    if (!Array.isArray(this.hir.imports) || !isRecord(this.hir.exports)) {
      throw new CodegenRequestError("Typed module HIR is missing imports or exports.");
    }
    for (const annotation of ir.annotations) {
      this.annotations.set(spanKey(annotation.span), annotation.ty);
    }
    for (const item of this.ast.items) {
      const kind = variantOf(item.kind);
      if (kind === "Export") {
        const declaration = structPayload<{ form: unknown }>(item.kind, "Export");
        if (variantOf(declaration.form) === "DefaultValue") {
          this.defaultExpression = payloadOf(declaration.form) as Expr;
        }
      }
    }
  }

  generate(): CodegenResult {
    if (this.hir.imports.length > 0) {
      this.unsupported(
        this.hir.imports[0]?.span,
        "Module imports are not implemented by the JavaScript backend yet.",
      );
    }
    for (const scope of this.ir.manualMemoryScopes) {
      this.diagnostic(
        "NS8003",
        "Manual memory is not supported by the JavaScript backend.",
        scope,
      );
    }

    const bodyParts: { readonly text: string; readonly span: Span }[] = [];
    for (const item of this.hir.items) {
      const generated = this.emitItem(item);
      if (generated.length > 0) {
        bodyParts.push({ text: generated, span: item.span });
      }
    }

    const exports = this.emitExports();
    if (exports.length > 0) {
      bodyParts.push({ text: exports, span: this.hir.items[0]?.span ?? { start: 0, end: 0 } });
    }
    const body = bodyParts.map((part) => part.text);

    const helperText = this.emitHelpers();
    const sections: string[] = [
      `// Generated by Neo Astra from ${this.ir.path}.`,
      "// Target: JavaScript ESM.",
    ];
    if (helperText.length > 0) {
      sections.push("", helperText);
    }
    if (body.length > 0) {
      sections.push("", body.join("\n\n"));
    }
    sections.push("");
    const code = sections.join("\n");
    const sourceMap =
      this.sourceText === undefined
        ? undefined
        : createSourceMap(code, this.ir.path, this.sourceText, bodyParts);

    return {
      code,
      diagnostics: this.diagnostics,
      ...(sourceMap === undefined ? {} : { sourceMap }),
    };
  }

  private emitItem(item: HirItem): string {
    const declaration = item.declaration;
    switch (variantOf(declaration)) {
      case "Function":
        return this.emitFunction(structPayload(declaration, "Function"));
      case "Struct":
        return this.emitStruct(structPayload(declaration, "Struct"));
      case "Enum":
        return this.emitEnum(structPayload(declaration, "Enum"));
      case "Constant":
        return this.emitConstant(structPayload(declaration, "Constant"));
      case "TypeAlias":
        return "";
      case "ForeignModule":
        this.unsupported(
          item.span,
          `Foreign module '${item.name}' requires a target adapter that the JavaScript backend does not have yet.`,
        );
        return "";
      default:
        this.unsupported(item.span, "Unknown resolved declaration kind.");
        return "";
    }
  }

  private emitFunction(declaration: {
    readonly name: string;
    readonly parameters: readonly { readonly name: string }[];
    readonly body: Block;
    readonly return_type?: AstType | null;
    readonly error_set?: readonly TypePath[];
  }): string {
    const name = jsName(declaration.name);
    const parameters = declaration.parameters.map((parameter) => jsName(parameter.name));
    const fallible = (declaration.error_set?.length ?? 0) > 0;
    const wrapsSuccess = fallible && !isResultReturnType(declaration.return_type);
    const previousWrap = this.currentSuccessWrap;
    this.currentSuccessWrap = wrapsSuccess;
    let body = this.emitBlock(declaration.body, "statement");
    if (containsVariant(declaration.body, "Spawn")) {
      this.useHelper("taskScope");
      const scope = this.temporary("taskScope");
      body = [
        "{",
        `  const ${scope} = $enterTaskScope();`,
        `  try ${body}`,
        "  finally {",
        `    $exitTaskScope(${scope});`,
        "  }",
        "}",
      ].join("\n");
    }
    this.currentSuccessWrap = previousWrap;
    const signature = `function ${name}(${parameters.join(", ")})`;
    if (!fallible) {
      return `${signature} ${body}`;
    }

    // A recoverable failure unwinds the current function like `?` does in the
    // language. Convert it to a value at the function boundary so callers can
    // match the error or propagate it again.
    const caught = this.temporary("caught");
    return [
      `${signature} {`,
      `  try ${body}`,
      `  catch (${caught}) {`,
      `    if (${caught} && ${caught}.$failure) return ${caught};`,
      `    throw ${caught};`,
      "  }",
      "}",
    ].join("\n");
  }

  private emitStruct(declaration: StructDecl): string {
    const name = jsName(declaration.name);
    const fields = declaration.fields;
    if (variantOf(fields) === "Named") {
      const named = payloadOf(fields) as readonly { readonly name: string }[];
      const body = named
        .map((field) => `    ${objectKey(field.name)}: fields${memberAccess(field.name)},`)
        .join("\n");
      return [
        `function ${name}(fields) {`,
        "  return {",
        `    $type: ${JSON.stringify(declaration.name)},`,
        body,
        "  };",
        "}",
      ]
        .filter((line) => line.length > 0)
        .join("\n");
    }

    const tuple = payloadOf(fields) as readonly unknown[];
    const parameters = tuple.map((_, index) => `value${index}`);
    const body = tuple
      .map((_, index) => `    ${index}: value${index},`)
      .join("\n");
    return [
      `function ${name}(${parameters.join(", ")}) {`,
      "  return {",
      `    $type: ${JSON.stringify(declaration.name)},`,
      body,
      "  };",
      "}",
    ]
      .filter((line) => line.length > 0)
      .join("\n");
  }

  private emitEnum(declaration: EnumDecl): string {
    const name = jsName(declaration.name);
    const variants = declaration.variants.map((variant) => {
      const fields = variant.fields;
      if (fields === null || fields.length === 0) {
        return `  ${objectKey(variant.name)}: Object.freeze({ $enum: ${JSON.stringify(declaration.name)}, $tag: ${JSON.stringify(variant.name)} }),`;
      }
      const parameters = fields.map((_, index) => `value${index}`);
      const values = fields.map((_, index) => `    ${index}: value${index},`).join("\n");
      return [
        `  ${objectKey(variant.name)}: (${parameters.join(", ")}) => ({`,
        `    $enum: ${JSON.stringify(declaration.name)},`,
        `    $tag: ${JSON.stringify(variant.name)},`,
        values,
        "  }),",
      ]
        .filter((line) => line.length > 0)
        .join("\n");
    });
    return [
      `const ${name} = Object.freeze({`,
      variants.join("\n"),
      "});",
    ].join("\n");
  }

  private emitConstant(declaration: {
    readonly name: string;
    readonly value: Expr;
  }): string {
    return `const ${jsName(declaration.name)} = ${this.emitExpr(declaration.value)};`;
  }

  private emitExports(): string {
    const lines: string[] = [];
    const surface = this.hir.exports;

    for (const [exported, entry] of Object.entries(surface.named)) {
      if (entry.kind === "Type") {
        continue;
      }
      const target = entry.target;
      if (variantOf(target) !== "Local") {
        this.unsupported(
          entry.span,
          `Re-export '${exported}' requires project linking, which the JavaScript backend does not implement yet.`,
        );
        continue;
      }
      const local = payloadOf(target);
      if (typeof local !== "string") {
        this.unsupported(entry.span, `Invalid local export target for '${exported}'.`);
        continue;
      }
      const localJs = jsName(local);
      const exportedJs = exportName(exported);
      lines.push(
        localJs === exported
          ? `export { ${localJs} };`
          : `export { ${localJs} as ${exportedJs} };`,
      );
    }

    const defaultExport = surface.default;
    if (defaultExport !== null) {
      const target = defaultExport.target;
      if (variantOf(target) === "Local") {
        const local = payloadOf(target);
        if (typeof local === "string") {
          lines.push(`export default ${jsName(local)};`);
        } else {
          this.unsupported(defaultExport.span, "Invalid default export target.");
        }
      } else if (variantOf(target) === "Expression") {
        if (this.defaultExpression === undefined) {
          this.unsupported(
            defaultExport.span,
            "The default export expression was missing from the typed AST.",
          );
        } else {
          lines.push(`export default ${this.emitExpr(this.defaultExpression)};`);
        }
      } else {
        this.unsupported(
          defaultExport.span,
          "The JavaScript backend does not implement this default export form yet.",
        );
      }
    }

    for (const star of surface.stars) {
      this.unsupported(
        star.span,
        "Star re-exports require project linking, which the JavaScript backend does not implement yet.",
      );
    }

    return lines.join("\n");
  }

  private emitHelpers(): string {
    if (this.helpers.size === 0) {
      return "";
    }
    return HELPER_ORDER.filter((name) => this.helpers.has(name))
      .map((name) => HELPERS[name].join("\n"))
      .join("\n\n");
  }

  private useHelper(name: HelperName): void {
    if (this.helpers.has(name)) {
      return;
    }
    this.helpers.add(name);
    for (const dependency of HELPER_DEPENDENCIES[name] ?? []) {
      this.useHelper(dependency);
    }
  }

  private emitBlock(block: Block, mode: "statement" | "value"): string {
    const inner = this.emitStatements(block.statements, mode);
    if (inner.length === 0) {
      return "{}";
    }
    return `{\n${indent(inner)}\n}`;
  }

  private emitStatements(
    statements: readonly Stmt[],
    mode: "statement" | "value",
  ): string {
    if (statements.length === 0) {
      return mode === "value" ? "return undefined;" : "";
    }

    const lines: string[] = [];
    let valueReturned = false;
    for (let index = 0; index < statements.length; index += 1) {
      const statement = statements[index];
      if (statement === undefined) {
        continue;
      }
      if (variantOf(statement.kind) === "Defer") {
        const deferred = payloadOf(statement.kind) as Stmt;
        const rest = this.emitStatements(statements.slice(index + 1), mode);
        const tryBody = rest.length === 0 ? "" : `\n${indent(rest)}\n`;
        const finallyBody = indent(this.emitStatement(deferred));
        lines.push(`try {${tryBody}} finally {\n${finallyBody}\n}`);
        return lines.join("\n");
      }

      const isLast = index === statements.length - 1;
      if (isLast && mode === "value" && variantOf(statement.kind) === "Expr") {
        const expression = payloadOf(statement.kind) as Expr;
        lines.push(`return ${this.emitExpr(expression)};`);
        valueReturned = true;
      } else {
        const emitted = this.emitStatement(statement);
        if (emitted.length > 0) {
          lines.push(emitted);
        }
      }
    }

    if (mode === "value" && !valueReturned) {
      lines.push("return undefined;");
    }
    return lines.join("\n");
  }

  private emitStatement(statement: Stmt): string {
    const kind = statement.kind;
    switch (variantOf(kind)) {
      case "Let": {
        const data = structPayload<{
          mutable: boolean;
          name: string;
          value: Expr;
        }>(kind, "Let");
        return `${data.mutable ? "let" : "const"} ${jsName(data.name)} = ${this.emitExpr(
          data.value,
        )};`;
      }
      case "Return": {
        const data = structPayload<{ value: Expr | null }>(kind, "Return");
        const value =
          data.value === null ? "undefined" : this.emitExpr(data.value);
        if (this.currentSuccessWrap) {
          this.useHelper("ok");
          return `return $ok(${value});`;
        }
        return data.value === null ? "return;" : `return ${value};`;
      }
      case "Break":
        return "break;";
      case "Continue":
        return "continue;";
      case "Defer": {
        this.unsupported(
          statement.span,
          "A defer statement must appear in a statement block.",
        );
        return "";
      }
      case "While": {
        const data = structPayload<{ condition: Expr; body: Block }>(kind, "While");
        return `while (${this.emitExpr(data.condition)}) ${this.emitBlock(
          data.body,
          "statement",
        )}`;
      }
      case "For": {
        const data = structPayload<{
          binding: string;
          iterable: Expr;
          body: Block;
        }>(kind, "For");
        return `for (const ${jsName(data.binding)} of ${this.emitIterable(
          data.iterable,
        )}) ${this.emitBlock(data.body, "statement")}`;
      }
      case "Expr":
        return `${this.emitExpr(payloadOf(kind) as Expr)};`;
      case "Scoped": {
        const data = structPayload<{
          annotation: { kind: "Mode" | "Memory"; value: string };
          statements: readonly Stmt[];
        }>(kind, "Scoped");
        const inner = this.emitStatements(data.statements, "statement");
        const comment = `// @${data.annotation.kind.toLowerCase()}:${data.annotation.value}`;
        return inner.length === 0
          ? `{\n${indent(comment)}\n}`
          : `{\n${indent(`${comment}\n${inner}`)}\n}`;
      }
      default:
        this.unsupported(statement.span, "Unknown statement kind.");
        return "";
    }
  }

  private emitIterable(expression: Expr): string {
    const type = this.typeAt(expression.span);
    if (type !== undefined && typeTag(type) === "Map") {
      this.useHelper("mapKeys");
      return `$mapKeys(${this.emitExpr(expression)})`;
    }
    const value = this.emitExpr(expression);
    if (type !== undefined && typeTag(type) === "Str") {
      return `Array.from(${value})`;
    }
    return value;
  }

  private emitExpr(expression: Expr): string {
    const kind = expression.kind;
    switch (variantOf(kind)) {
      case "Literal":
        return this.emitLiteral(payloadOf(kind) as Literal, expression.span);
      case "Identifier":
        return this.emitIdentifier(payloadOf(kind) as string, expression.span);
      case "Unary": {
        const data = structPayload<{
          operator: "Negate" | "Not";
          operand: Expr;
        }>(kind, "Unary");
        const operand = this.emitExpr(data.operand);
        if (data.operator === "Not") {
          return `!(${operand})`;
        }
        const type = this.typeAt(expression.span);
        const wrapped = type === undefined ? `-(${operand})` : this.wrapNumber(type, `-(${operand})`, expression.span);
        return wrapped ?? `-(${operand})`;
      }
      case "Binary":
        return this.emitBinary(
          structPayload<{
            operator: string;
            left: Expr;
            right: Expr;
          }>(kind, "Binary"),
          expression.span,
        );
      case "Assign": {
        const data = structPayload<{ target: Expr; value: Expr }>(kind, "Assign");
        return `${this.emitExpr(data.target)} = ${this.emitExpr(data.value)}`;
      }
      case "Call":
        return this.emitCall(
          structPayload<{ callee: Expr; arguments: readonly Expr[] }>(kind, "Call"),
          expression.span,
        );
      case "Index": {
        const data = structPayload<{ target: Expr; index: Expr }>(kind, "Index");
        return this.emitIndex(data.target, data.index);
      }
      case "Member": {
        const data = structPayload<{ target: Expr; member: string }>(kind, "Member");
        return this.emitMember(data.target, data.member);
      }
      case "TypeArguments":
        return this.emitExpr(
          structPayload<{ target: Expr }>(kind, "TypeArguments").target,
        );
      case "Propagate": {
        const inner = payloadOf(kind) as Expr;
        const value = this.emitExpr(inner);
        const temporary = this.temporary("propagate");
        this.useHelper("failure");
        return [
          "(() => {",
          `  const ${temporary} = ${value};`,
          `  if (${temporary} && ${temporary}.$failure) throw ${temporary};`,
          `  if (${temporary} && ${temporary}.$result === true && !${temporary}.ok) throw $failure(${temporary}.error);`,
          `  return ${temporary} && ${temporary}.$result === true ? ${temporary}.value : ${temporary};`,
          "})()",
        ].join("\n");
      }
      case "Array": {
        const values = payloadOf(kind) as readonly Expr[];
        return `[${values.map((value) => this.emitExpr(value)).join(", ")}]`;
      }
      case "Map": {
        const entries = payloadOf(kind) as readonly {
          readonly key: Expr;
          readonly value: Expr;
        }[];
        this.useHelper("map");
        return `$map([${entries
          .map((entry) => `[${this.emitExpr(entry.key)}, ${this.emitExpr(entry.value)}]`)
          .join(", ")}])`;
      }
      case "Tuple": {
        const values = payloadOf(kind) as readonly Expr[];
        this.useHelper("tuple");
        return `$tuple([${values.map((value) => this.emitExpr(value)).join(", ")}])`;
      }
      case "StructInit": {
        const data = structPayload<{
          path: TypePath;
          fields: readonly { readonly name: string; readonly value: Expr }[];
        }>(kind, "StructInit");
        const name = data.path.segments[data.path.segments.length - 1] ?? "Anonymous";
        const fields = data.fields
          .map(
            (field) =>
              `    ${objectKey(field.name)}: ${this.emitExpr(field.value)},`,
          )
          .join("\n");
        return `${jsName(name)}({\n${fields}\n})`;
      }
      case "Block":
        return this.emitBlockExpression(payloadOf(kind) as Block);
      case "If": {
        const data = structPayload<{
          condition: Expr;
          then_branch: Block;
          else_branch: unknown;
        }>(kind, "If");
        return this.emitIfExpression(data);
      }
      case "Match": {
        const data = structPayload<{
          value: Expr;
          arms: readonly MatchArm[];
        }>(kind, "Match");
        return this.emitMatchExpression(data.value, data.arms);
      }
      case "Spawn": {
        const block = structPayload<{ body: Block }>(kind, "Spawn").body;
        const previousWrap = this.currentSuccessWrap;
        this.currentSuccessWrap = false;
        const body = this.emitBlock(block, "value");
        this.currentSuccessWrap = previousWrap;
        this.useHelper("spawn");
        return `$spawn(() => ${body})`;
      }
      case "Fail": {
        const data = structPayload<{ error: TypePath; arguments: unknown }>(
          kind,
          "Fail",
        );
        this.useHelper("fail");
        return `$fail(${this.emitFailureValue(
          data.error,
          data.arguments,
          expression.span,
        )})`;
      }
      case "Panic": {
        const message = structPayload<{ message: Expr }>(kind, "Panic").message;
        this.useHelper("panic");
        this.useHelper("render");
        return `$panic($render(${this.emitExpr(message)}))`;
      }
      case "Abort": {
        this.unsupported(
          expression.span,
          "Abort without unwinding is not representable in JavaScript yet.",
        );
        return "undefined";
      }
      default:
        this.unsupported(expression.span, "Unknown expression kind.");
        return "undefined";
    }
  }

  private emitLiteral(literal: Literal, span: Span): string {
    switch (variantOf(literal)) {
      case "Integer": {
        const raw = payloadOf(literal);
        const value = stripIntegerSuffix(String(raw));
        const type = this.typeAt(span);
        if (type !== undefined && BIG_INTEGER_TAGS.has(typeTag(type))) {
          return `BigInt(${JSON.stringify(value)})`;
        }
        return value;
      }
      case "Float": {
        const raw = stripUnderscores(String(payloadOf(literal)));
        const type = this.typeAt(span);
        this.useHelper("float");
        if (type !== undefined && typeTag(type) === "F32") {
          return `$float(Math.fround(${raw}))`;
        }
        return `$float(${raw})`;
      }
      case "String":
        return JSON.stringify(payloadOf(literal));
      case "Bytes": {
        const bytes = payloadOf(literal) as readonly number[];
        return `new Uint8Array([${bytes.join(", ")}])`;
      }
      case "Bool":
        return payloadOf(literal) === true ? "true" : "false";
      case "Unit":
        return "undefined";
      default:
        this.unsupported(span, "Unknown literal kind.");
        return "undefined";
    }
  }

  private emitFailureValue(path: TypePath, argumentsValue: unknown, span: Span): string {
    const name = path.segments[path.segments.length - 1] ?? "";
    const values =
      variantOf(argumentsValue) === "Tuple"
        ? ((payloadOf(argumentsValue) as readonly Expr[]).map((value) =>
            this.emitExpr(value),
          ))
        : ((payloadOf(argumentsValue) as readonly {
            readonly name: string;
            readonly value: Expr;
          }[]).map((field) => this.emitExpr(field.value)));

    const item = this.hir.items.find((candidate) => candidate.name === name);
    if (item === undefined) {
      // Built-in nominal errors such as ParseIntError and TaskError use the
      // same tagged shape that the collection/string helpers produce.
      const fields = values.map((value, index) => `${index}: ${value}`);
      return `{ $type: ${JSON.stringify(name)}${fields.length > 0 ? `, ${fields.join(", ")}` : ""} }`;
    }

    if (item.kind === "Enum") {
      const declaration = structPayload<EnumDecl>(item.declaration, "Enum");
      const variant = declaration.variants[0]?.name;
      if (variant === undefined) {
        this.unsupported(span, `Failure enum '${name}' has no constructible variant.`);
        return "undefined";
      }
      const constructed = `${jsName(name)}.${objectKey(variant)}`;
      return values.length === 0 ? constructed : `${constructed}(${values.join(", ")})`;
    }

    if (variantOf(argumentsValue) === "Struct") {
      const fields = payloadOf(argumentsValue) as readonly {
        readonly name: string;
        readonly value: Expr;
      }[];
      const initializers = fields
        .map(
          (field) => `    ${objectKey(field.name)}: ${this.emitExpr(field.value)},`,
        )
        .join("\n");
      return `${jsName(name)}({\n${initializers}\n})`;
    }

    return `${jsName(name)}(${values.join(", ")})`;
  }

  private emitIdentifier(name: string, _span: Span): string {
    if (name === "None") {
      this.useHelper("none");
      return "$none";
    }
    switch (name) {
      case "print":
        this.useHelper("print");
        return "$print";
      case "println":
        this.useHelper("println");
        return "$println";
      case "dbg":
        this.useHelper("dbg");
        return "$dbg";
      case "assert":
        this.useHelper("assert");
        return "$assert";
      default:
        return jsName(name);
    }
  }

  private emitBinary(
    data: { readonly operator: string; readonly left: Expr; readonly right: Expr },
    span: Span,
  ): string {
    const left = this.emitExpr(data.left);
    const right = this.emitExpr(data.right);
    switch (data.operator) {
      case "And":
        return `(${left} && ${right})`;
      case "Or":
        return `(${left} || ${right})`;
      case "Equal":
        this.useHelper("eq");
        return `$eq(${left}, ${right})`;
      case "NotEqual":
        this.useHelper("eq");
        return `!$eq(${left}, ${right})`;
      case "Less":
        return `(${left} < ${right})`;
      case "LessEqual":
        return `(${left} <= ${right})`;
      case "Greater":
        return `(${left} > ${right})`;
      case "GreaterEqual":
        return `(${left} >= ${right})`;
      default:
        break;
    }

    const type = this.typeAt(span);
    const tag = type === undefined ? undefined : typeTag(type);
    if (tag !== undefined && INTEGER_TAGS.has(tag)) {
      const wrap = data.operator.endsWith("Wrap");
      const operator = data.operator.replace(/Wrap$/, "");
      return this.emitIntegerBinary(tag, operator, left, right, wrap, span);
    }
    if (tag === "F32" || tag === "F64") {
      this.useHelper("float");
      let value: string;
      switch (data.operator) {
        case "Add":
        case "AddWrap":
          value = `(${left} + ${right})`;
          break;
        case "Subtract":
        case "SubtractWrap":
          value = `(${left} - ${right})`;
          break;
        case "Multiply":
        case "MultiplyWrap":
          value = `(${left} * ${right})`;
          break;
        case "Divide":
        case "DivideWrap":
          value = `(${left} / ${right})`;
          break;
        case "Remainder":
          value = `(${left} % ${right})`;
          break;
        default:
          this.unsupported(span, `Unsupported numeric operator '${data.operator}'.`);
          return "undefined";
      }
      return tag === "F32" ? `$float(Math.fround(${value}))` : `$float(${value})`;
    }

    switch (data.operator) {
      case "Add":
      case "AddWrap":
        return `(${left} + ${right})`;
      case "Subtract":
      case "SubtractWrap":
        return `(${left} - ${right})`;
      case "Multiply":
      case "MultiplyWrap":
        return `(${left} * ${right})`;
      case "Divide":
      case "DivideWrap":
        this.useHelper("div");
        return `$div(${left}, ${right})`;
      case "Remainder":
        this.useHelper("rem");
        return `$rem(${left}, ${right})`;
      default:
        this.unsupported(span, `Unsupported binary operator '${data.operator}'.`);
        return "undefined";
    }
  }

  private emitIntegerBinary(
    tag: string,
    operator: string,
    left: string,
    right: string,
    wrapping: boolean,
    span: Span,
  ): string {
    if (this.ir.integerPolicy === "arbitrary" && !wrapping) {
      this.unsupported(
        span,
        "The 'arbitrary' integer policy is not implemented by the JavaScript backend yet.",
      );
    }
    const big = BIG_INTEGER_TAGS.has(tag);
    const checked =
      !wrapping && this.ir.integerPolicy === "safe-checked";
    let raw: string;
    switch (operator) {
      case "Add":
        raw = `(${left} + ${right})`;
        break;
      case "Subtract":
        raw = `(${left} - ${right})`;
        break;
      case "Multiply":
        raw = `(${left} * ${right})`;
        break;
      case "Divide":
        if (big) {
          this.useHelper("divBig");
          raw = `$divBig(${left}, ${right})`;
        } else {
          this.useHelper("div");
          raw = `$div(${left}, ${right})`;
        }
        break;
      case "Remainder":
        if (big) {
          this.useHelper("remBig");
          raw = `$remBig(${left}, ${right})`;
        } else {
          this.useHelper("rem");
          raw = `$rem(${left}, ${right})`;
        }
        break;
      default:
        this.unsupported(span, `Unsupported integer operator '${operator}'.`);
        return "undefined";
    }

    if (checked) {
      const helper = `checked_${tag.toLowerCase()}` as HelperName;
      if (HELPER_ORDER.includes(helper)) {
        this.useHelper(helper);
        return `${helperName(helper)}(${raw})`;
      }
    }

    const helper = tag.toLowerCase() as HelperName;
    if (HELPER_ORDER.includes(helper)) {
      this.useHelper(helper);
      return `${helperName(helper)}(${raw})`;
    }
    return raw;
  }

  private wrapNumber(
    type: SemanticType,
    value: string,
    span: Span,
  ): string | undefined {
    const tag = typeTag(type);
    if (tag === "F32") {
      this.useHelper("float");
      return `$float(Math.fround(${value}))`;
    }
    if (tag === "F64") {
      this.useHelper("float");
      return `$float(${value})`;
    }
    if (tag === "Usize" || tag === "Isize") {
      return value;
    }
    if (this.ir.integerPolicy === "arbitrary" && INTEGER_TAGS.has(tag)) {
      this.unsupported(
        span,
        "The 'arbitrary' integer policy is not implemented by the JavaScript backend yet.",
      );
    }
    if (this.ir.integerPolicy === "safe-checked" && INTEGER_TAGS.has(tag)) {
      const checked = `checked_${tag.toLowerCase()}` as HelperName;
      if (HELPER_ORDER.includes(checked)) {
        this.useHelper(checked);
        return `${helperName(checked)}(${value})`;
      }
    }
    const helper = tag.toLowerCase() as HelperName;
    if (HELPER_ORDER.includes(helper)) {
      this.useHelper(helper);
      return `${helperName(helper)}(${value})`;
    }
    return undefined;
  }

  private emitCall(
    data: { readonly callee: Expr; readonly arguments: readonly Expr[] },
    span: Span,
  ): string {
    const args = data.arguments.map((argument) => this.emitExpr(argument));
    const calleeKind = variantOf(data.callee.kind);

    if (calleeKind === "Identifier") {
      const name = payloadOf(data.callee.kind) as string;
      switch (name) {
        case "print":
          this.useHelper("print");
          return `$print(${args[0] ?? "undefined"})`;
        case "println":
          this.useHelper("println");
          return `$println(${args[0] ?? "undefined"})`;
        case "dbg":
          this.useHelper("dbg");
          return `$dbg(${args[0] ?? "undefined"})`;
        case "assert":
          this.useHelper("assert");
          return `$assert(${args[0] ?? "false"})`;
        case "Some":
          if (args.length !== 1) {
            this.unsupported(span, "Some expects exactly one argument.");
            this.useHelper("none");
            return "$none";
          }
          this.useHelper("some");
          return `$some(${args[0] ?? "undefined"})`;
        case "Ok":
          this.useHelper("ok");
          return `$ok(${args[0] ?? "undefined"})`;
        case "Err":
          this.useHelper("err");
          return `$err(${args[0] ?? "undefined"})`;
        default:
          return `${this.emitExpr(data.callee)}(${args.join(", ")})`;
      }
    }

    if (calleeKind === "Member") {
      const member = structPayload<{ target: Expr; member: string }>(
        data.callee.kind,
        "Member",
      );
      const emitted = this.emitMethodCall(member.target, member.member, args, span);
      if (emitted !== undefined) {
        return emitted;
      }
      return `${this.emitExpr(data.callee)}(${args.join(", ")})`;
    }

    if (calleeKind === "TypeArguments") {
      const target = structPayload<{ target: Expr }>(data.callee.kind, "TypeArguments");
      return this.emitCall({ callee: target.target, arguments: data.arguments }, span);
    }

    return `${this.emitExpr(data.callee)}(${args.join(", ")})`;
  }

  private emitMethodCall(
    receiver: Expr,
    method: string,
    args: readonly string[],
    span: Span,
  ): string | undefined {
    const receiverType = this.typeAt(receiver.span);
    const tag = receiverType === undefined ? undefined : typeTag(receiverType);
    const target = this.emitExpr(receiver);

    if (method === "len") {
      this.useHelper("len");
      return `$len(${target})`;
    }
    if (method === "is_empty") {
      this.useHelper("len");
      return `($len(${target}) === 0)`;
    }
    if (method === "push" && (tag === "Array" || tag === "Slice")) {
      return `(${target}.push(${args[0] ?? "undefined"}), undefined)`;
    }
    if (method === "get") {
      if (tag === "Map") {
        this.useHelper("mapGet");
        return `$mapGet(${target}, ${args[0] ?? "undefined"})`;
      }
      this.useHelper("getArray");
      return `$getArray(${target}, ${args[0] ?? "undefined"})`;
    }
    if (method === "insert" && tag === "Map") {
      this.useHelper("mapInsert");
      return `$mapInsert(${target}, ${args[0] ?? "undefined"}, ${args[1] ?? "undefined"})`;
    }
    if (method === "to_int" && tag === "Str") {
      this.useHelper("parseInt");
      return `$parseInt(${target})`;
    }
    if (tag === "Named") {
      const named = payloadOf(receiverType) as readonly [string, ...unknown[]];
      if (named[0] === "Task") {
        switch (method) {
          case "join":
            this.useHelper("taskJoin");
            return `$taskJoin(${target})`;
          case "cancel":
            this.useHelper("taskCancel");
            return `$taskCancel(${target})`;
          default:
            break;
        }
      }
    }
    if (tag === "Option") {
      switch (method) {
        case "is_some":
          return `${target}.$option`;
        case "is_none":
          return `!${target}.$option`;
        case "unwrap":
          this.useHelper("unwrapOption");
          return `$unwrapOption(${target})`;
        case "unwrap_or":
          return `(${target}.$option ? ${target}.value : ${args[0] ?? "undefined"})`;
        default:
          break;
      }
    }
    if (tag === "Result") {
      switch (method) {
        case "is_ok":
          return `${target}.ok`;
        case "is_err":
          return `!${target}.ok`;
        case "unwrap":
          this.useHelper("unwrapResult");
          return `$unwrapResult(${target})`;
        case "unwrap_or":
          return `(${target}.ok ? ${target}.value : ${args[0] ?? "undefined"})`;
        case "error":
          this.useHelper("resultError");
          return `$resultError(${target})`;
        default:
          break;
      }
    }
    if (tag !== undefined && INTEGER_TAGS.has(tag)) {
      const wrapped = this.emitIntegerMethod(tag, method, target, args[0], span);
      if (wrapped !== undefined) {
        return wrapped;
      }
    }

    return undefined;
  }

  private emitIntegerMethod(
    tag: string,
    method: string,
    receiver: string,
    argument: string | undefined,
    span: Span,
  ): string | undefined {
    const match = /^(wrapping|checked)_(add|sub|mul|div)$/.exec(method);
    if (match === null) {
      return undefined;
    }
    const kind = match[1];
    const operation = match[2];
    if (argument === undefined) {
      this.unsupported(span, `'${method}' expects one argument.`);
      return "undefined";
    }
    if (kind === "checked") {
      this.useHelper("checkedInteger");
      return `$checkedInteger(${JSON.stringify(tag.toLowerCase())}, ${JSON.stringify(operation)}, ${receiver}, ${argument})`;
    }
    const operator =
      operation === "add" ? "Add" : operation === "sub" ? "Subtract" : operation === "mul" ? "Multiply" : "Divide";
    return this.emitIntegerBinary(tag, operator, receiver, argument, true, span);
  }

  private emitIndex(target: Expr, index: Expr): string {
    const targetText = this.emitExpr(target);
    const indexText = this.emitExpr(index);
    const type = this.typeAt(target.span);
    const tag = type === undefined ? undefined : typeTag(type);
    if (tag === "Tuple") {
      this.useHelper("tupleAt");
      return `$tupleAt(${targetText}, ${indexText})`;
    }
    if (tag === "Str") {
      this.useHelper("strAt");
      return `$strAt(${targetText}, ${indexText})`;
    }
    if (tag === "Map") {
      this.unsupported(
        target.span,
        "Map index syntax is not implemented by the JavaScript backend; use get().",
      );
      return "undefined";
    }
    if (tag !== undefined && isSequence(tag)) {
      this.useHelper("index");
      return `$index(${targetText}, ${indexText})`;
    }
    return `${targetText}[${indexText}]`;
  }

  private emitMember(target: Expr, member: string): string {
    const targetText = this.emitExpr(target);
    const type = this.typeAt(target.span);
    const tag = type === undefined ? undefined : typeTag(type);
    if (/^[0-9]+$/.test(member)) {
      if (tag === "Tuple") {
        this.useHelper("tupleAt");
        return `$tupleAt(${targetText}, ${Number(member)})`;
      }
      return `${targetText}[${Number(member)}]`;
    }
    return `${targetText}${memberAccess(member)}`;
  }

  private emitBlockExpression(block: Block): string {
    if (this.scanStatements(block.statements, 0)) {
      this.unsupported(
        block.span,
        "Control flow that exits an expression block is not implemented by the JavaScript backend yet.",
      );
      return "undefined";
    }
    return `(() => ${this.emitBlock(block, "value")})()`;
  }

  private emitIfExpression(data: {
    readonly condition: Expr;
    readonly then_branch: Block;
    readonly else_branch: unknown;
  }): string {
    if (this.scanStatements(data.then_branch.statements, 0)) {
      this.unsupported(
        data.then_branch.span,
        "Control flow that exits an expression block is not implemented by the JavaScript backend yet.",
      );
      return "undefined";
    }
    if (
      data.else_branch !== null &&
      data.else_branch !== undefined &&
      variantOf(data.else_branch) === "Block"
    ) {
      const block = payloadOf(data.else_branch) as Block;
      if (this.scanStatements(block.statements, 0)) {
        this.unsupported(
          block.span,
          "Control flow that exits an expression block is not implemented by the JavaScript backend yet.",
        );
        return "undefined";
      }
    }
    const condition = this.emitExpr(data.condition);
    const thenBranch = this.emitBlock(data.then_branch, "value");
    let otherwise = "";
    if (data.else_branch !== null && data.else_branch !== undefined) {
      if (variantOf(data.else_branch) === "Block") {
        otherwise = ` else ${this.emitBlock(payloadOf(data.else_branch) as Block, "value")}`;
      } else {
        otherwise = ` else return ${this.emitExpr(payloadOf(data.else_branch) as Expr)};`;
      }
    }
    return `(() => {\n  if (${condition}) ${thenBranch}${otherwise}\n})()`;
  }

  private emitMatchExpression(value: Expr, arms: readonly MatchArm[]): string {
    if (this.scanExpression(value, 0)) {
      this.unsupported(
        value.span,
        "Control flow that exits an expression block is not implemented by the JavaScript backend yet.",
      );
      return "undefined";
    }
    for (const arm of arms) {
      if (variantOf(arm.body) === "Block") {
        const block = payloadOf(arm.body) as Block;
        if (this.scanStatements(block.statements, 0)) {
          this.unsupported(
            block.span,
            "Control flow that exits an expression block is not implemented by the JavaScript backend yet.",
          );
          return "undefined";
        }
      } else if (this.scanExpression(payloadOf(arm.body) as Expr, 0)) {
        this.unsupported(
          arm.span,
          "Control flow that exits an expression block is not implemented by the JavaScript backend yet.",
        );
        return "undefined";
      }
    }

    const temporary = this.temporary("match");
    this.useHelper("err");
    const lines = [
      `let ${temporary} = ${this.emitExpr(value)};`,
      `if (${temporary} && ${temporary}.$failure) ${temporary} = $err(${temporary}.error);`,
    ];
    let hasCatchAll = false;

    for (const arm of arms) {
      if (hasCatchAll) {
        continue;
      }
      const pattern = this.lowerPattern(arm.pattern, temporary);
      const body =
        variantOf(arm.body) === "Expr"
          ? `return ${this.emitExpr(payloadOf(arm.body) as Expr)};`
          : this.emitBlock(payloadOf(arm.body) as Block, "value");
      const bindings =
        pattern.bindings.length === 0 ? "" : `${indent(pattern.bindings.join("\n"))}\n`;
      if (pattern.checks.length === 0) {
        hasCatchAll = true;
        lines.push(`{\n${bindings}${indent(body)}\n}`);
      } else {
        lines.push(
          `if (${pattern.checks.join(" && ")}) {\n${bindings}${indent(body)}\n}`,
        );
      }
    }

    this.useHelper("panic");
    lines.push('$panic("non-exhaustive match");');
    return `(() => {\n${indent(lines.join("\n"))}\n})()`;
  }

  private scanStatements(statements: readonly Stmt[], loopDepth: number): boolean {
    for (const statement of statements) {
      if (this.scanStatement(statement, loopDepth)) {
        return true;
      }
    }
    return false;
  }

  private scanStatement(statement: Stmt, loopDepth: number): boolean {
    const kind = statement.kind;
    switch (variantOf(kind)) {
      case "Return":
        return true;
      case "Break":
      case "Continue":
        return loopDepth === 0;
      case "Let":
        return this.scanExpression(
          structPayload<{ value: Expr }>(kind, "Let").value,
          loopDepth,
        );
      case "Defer":
        return this.scanStatement(payloadOf(kind) as Stmt, loopDepth);
      case "While": {
        const data = structPayload<{ condition: Expr; body: Block }>(kind, "While");
        return (
          this.scanExpression(data.condition, loopDepth) ||
          this.scanStatements(data.body.statements, loopDepth + 1)
        );
      }
      case "For": {
        const data = structPayload<{ iterable: Expr; body: Block }>(kind, "For");
        return (
          this.scanExpression(data.iterable, loopDepth) ||
          this.scanStatements(data.body.statements, loopDepth + 1)
        );
      }
      case "Expr":
        return this.scanExpression(payloadOf(kind) as Expr, loopDepth);
      case "Scoped": {
        const data = structPayload<{ statements: readonly Stmt[] }>(kind, "Scoped");
        return this.scanStatements(data.statements, loopDepth);
      }
      default:
        return false;
    }
  }

  private scanExpression(expression: Expr, loopDepth: number): boolean {
    const kind = expression.kind;
    switch (variantOf(kind)) {
      case "Literal":
      case "Identifier":
        return false;
      case "Unary":
        return this.scanExpression(
          structPayload<{ operand: Expr }>(kind, "Unary").operand,
          loopDepth,
        );
      case "Binary": {
        const data = structPayload<{ left: Expr; right: Expr }>(kind, "Binary");
        return (
          this.scanExpression(data.left, loopDepth) ||
          this.scanExpression(data.right, loopDepth)
        );
      }
      case "Assign": {
        const data = structPayload<{ target: Expr; value: Expr }>(kind, "Assign");
        return (
          this.scanExpression(data.target, loopDepth) ||
          this.scanExpression(data.value, loopDepth)
        );
      }
      case "Call": {
        const data = structPayload<{
          callee: Expr;
          arguments: readonly Expr[];
        }>(kind, "Call");
        return (
          this.scanExpression(data.callee, loopDepth) ||
          data.arguments.some((argument) => this.scanExpression(argument, loopDepth))
        );
      }
      case "Index": {
        const data = structPayload<{ target: Expr; index: Expr }>(kind, "Index");
        return (
          this.scanExpression(data.target, loopDepth) ||
          this.scanExpression(data.index, loopDepth)
        );
      }
      case "Member":
        return this.scanExpression(
          structPayload<{ target: Expr }>(kind, "Member").target,
          loopDepth,
        );
      case "TypeArguments":
        return this.scanExpression(
          structPayload<{ target: Expr }>(kind, "TypeArguments").target,
          loopDepth,
        );
      case "Propagate":
        return this.scanExpression(payloadOf(kind) as Expr, loopDepth);
      case "Array":
      case "Tuple": {
        const values = payloadOf(kind) as readonly Expr[];
        return values.some((value) => this.scanExpression(value, loopDepth));
      }
      case "Map": {
        const entries = payloadOf(kind) as readonly {
          readonly key: Expr;
          readonly value: Expr;
        }[];
        return entries.some(
          (entry) =>
            this.scanExpression(entry.key, loopDepth) ||
            this.scanExpression(entry.value, loopDepth),
        );
      }
      case "StructInit": {
        const fields = structPayload<{
          fields: readonly { readonly value: Expr }[];
        }>(kind, "StructInit").fields;
        return fields.some((field) => this.scanExpression(field.value, loopDepth));
      }
      case "Block":
        return this.scanStatements((payloadOf(kind) as Block).statements, loopDepth);
      case "If": {
        const data = structPayload<{
          condition: Expr;
          then_branch: Block;
          else_branch: unknown;
        }>(kind, "If");
        if (
          this.scanExpression(data.condition, loopDepth) ||
          this.scanStatements(data.then_branch.statements, loopDepth)
        ) {
          return true;
        }
        if (data.else_branch === null || data.else_branch === undefined) {
          return false;
        }
        return variantOf(data.else_branch) === "Block"
          ? this.scanStatements(
              (payloadOf(data.else_branch) as Block).statements,
              loopDepth,
            )
          : this.scanExpression(payloadOf(data.else_branch) as Expr, loopDepth);
      }
      case "Match": {
        const data = structPayload<{
          value: Expr;
          arms: readonly MatchArm[];
        }>(kind, "Match");
        if (this.scanExpression(data.value, loopDepth)) {
          return true;
        }
        return data.arms.some((arm) =>
          variantOf(arm.body) === "Block"
            ? this.scanStatements(
                (payloadOf(arm.body) as Block).statements,
                loopDepth,
              )
            : this.scanExpression(payloadOf(arm.body) as Expr, loopDepth),
        );
      }
      case "Spawn":
        return false;
      case "Fail": {
        const args = structPayload<{ arguments: unknown }>(kind, "Fail").arguments;
        if (variantOf(args) === "Tuple") {
          return (payloadOf(args) as readonly Expr[]).some((arg) =>
            this.scanExpression(arg, loopDepth),
          );
        }
        return (payloadOf(args) as readonly { readonly value: Expr }[]).some(
          (field) => this.scanExpression(field.value, loopDepth),
        );
      }
      case "Panic":
      case "Abort":
        return this.scanExpression(
          structPayload<{ message: Expr }>(kind, variantOf(kind)).message,
          loopDepth,
        );
      default:
        return false;
    }
  }

  private lowerPattern(
    pattern: Pattern,
    value: string,
  ): { readonly checks: readonly string[]; readonly bindings: readonly string[] } {
    const checks: string[] = [];
    const bindings: string[] = [];
    this.lowerPatternInto(pattern, value, checks, bindings);
    return { checks, bindings };
  }

  private lowerPatternInto(
    pattern: Pattern,
    value: string,
    checks: string[],
    bindings: string[],
  ): void {
    const kind = pattern.kind;
    const tag = variantOf(kind);
    switch (tag) {
      case "Wildcard":
        return;
      case "Identifier": {
        const name = payloadOf(kind);
        if (typeof name !== "string") {
          return;
        }
        if (name === "None") {
          checks.push(`${value}.$option === false`);
          return;
        }
        bindings.push(`const ${jsName(name)} = ${value};`);
        return;
      }
      case "Literal": {
        this.useHelper("eq");
        checks.push(
          `$eq(${value}, ${this.emitLiteral(payloadOf(kind) as Literal, pattern.span)})`,
        );
        return;
      }
      case "Tuple": {
        const items = payloadOf(kind) as readonly Pattern[];
        checks.push(`Array.isArray(${value})`);
        checks.push(`${value}.length === ${items.length}`);
        items.forEach((item, index) => {
          this.lowerPatternInto(item, `${value}[${index}]`, checks, bindings);
        });
        return;
      }
      case "Constructor": {
        const data = structPayload<{
          path: TypePath;
          arguments: readonly Pattern[];
        }>(kind, "Constructor");
        const name = data.path.segments[data.path.segments.length - 1] ?? "";
        if (name === "Some") {
          checks.push(`${value}.$option === true`);
          const argument = data.arguments[0];
          if (argument !== undefined) {
            this.lowerPatternInto(argument, `${value}.value`, checks, bindings);
          }
          return;
        }
        if (name === "Ok" || name === "Err") {
          checks.push(`${value}.ok === ${name === "Ok" ? "true" : "false"}`);
          const argument = data.arguments[0];
          if (argument !== undefined) {
            this.lowerPatternInto(
              argument,
              `${value}.${name === "Ok" ? "value" : "error"}`,
              checks,
              bindings,
            );
          }
          return;
        }
        checks.push(`${value} !== null && ${value}.$type === ${JSON.stringify(name)}`);
        data.arguments.forEach((argument, index) => {
          this.lowerPatternInto(argument, `${value}[${index}]`, checks, bindings);
        });
        return;
      }
      case "Variant": {
        const data = structPayload<{
          variant: string;
          arguments: readonly Pattern[] | null;
        }>(kind, "Variant");
        checks.push(`${value} !== null && ${value}.$tag === ${JSON.stringify(data.variant)}`);
        if (data.arguments !== null) {
          data.arguments.forEach((argument, index) => {
            this.lowerPatternInto(argument, `${value}[${index}]`, checks, bindings);
          });
        }
        return;
      }
      default:
        this.unsupported(pattern.span, "Unknown pattern kind.");
    }
  }

  private typeAt(span: Span): SemanticType | undefined {
    return this.annotations.get(spanKey(span));
  }

  private temporary(label: string): string {
    this.tempIndex += 1;
    return `$${label}${this.tempIndex}`;
  }

  private unsupported(span: Span | undefined, message: string): void {
    this.diagnostic("NS8001", message, span);
  }

  private diagnostic(code: string, message: string, span: Span | undefined): void {
    const key = span === undefined ? `${code}:` : `${code}:${span.start}:${span.end}`;
    if (this.reported.has(key)) {
      return;
    }
    this.reported.add(key);
    this.diagnostics.push(
      span === undefined ? { code, message } : { code, message, span },
    );
  }
}

/** Generates readable JavaScript ESM for one typed module. */
export function generateJavaScript(request: CodegenRequest): CodegenResult {
  const ir = parseModuleIr(request.ir);
  return new JavaScriptGenerator(ir, request.sourceText).generate();
}

function parseModuleIr(value: unknown): ModuleIr {
  if (!isRecord(value)) {
    throw new CodegenRequestError("Typed module request must be an object.");
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
    throw new CodegenRequestError("Typed module request has an invalid shape.");
  }
  return value as unknown as ModuleIr;
}

function spanKey(span: Span): string {
  return `${span.start}:${span.end}`;
}

function isSequence(tag: string): boolean {
  return tag === "Array" || tag === "FixedArray" || tag === "Slice" || tag === "Bytes";
}

function createSourceMap(
  code: string,
  sourcePath: string,
  sourceText: string,
  anchors: readonly { readonly text: string; readonly span: Span }[],
): string {
  const mappedLines = new Map<number, number>();
  let searchFrom = 0;
  for (const anchor of anchors) {
    const index = code.indexOf(anchor.text, searchFrom);
    if (index < 0) {
      continue;
    }
    searchFrom = index + anchor.text.length;
    const generatedLine = lineAt(code, index);
    if (!mappedLines.has(generatedLine)) {
      mappedLines.set(generatedLine, lineAt(sourceText, anchor.span.start));
    }
  }

  const lines = code.split("\n");
  let previousSource = 0;
  let previousSourceLine = 0;
  let previousSourceColumn = 0;
  const mappings = lines.map((_, generatedLine) => {
    const sourceLine = mappedLines.get(generatedLine);
    if (sourceLine === undefined) {
      return "";
    }
    const segment =
      encodeVlq(0) +
      encodeVlq(0 - previousSource) +
      encodeVlq(sourceLine - previousSourceLine) +
      encodeVlq(0 - previousSourceColumn);
    previousSource = 0;
    previousSourceLine = sourceLine;
    previousSourceColumn = 0;
    return segment;
  });

  return JSON.stringify({
    version: 3,
    sources: [sourcePath],
    sourcesContent: [sourceText],
    names: [],
    mappings: mappings.join(";"),
  });
}

function lineAt(text: string, offset: number): number {
  let line = 0;
  for (let index = 0; index < offset && index < text.length; index += 1) {
    if (text[index] === "\n") {
      line += 1;
    }
  }
  return line;
}

const BASE64_VLQ =
  "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

function encodeVlq(value: number): string {
  let remaining = value < 0 ? ((-value) << 1) | 1 : value << 1;
  let output = "";
  do {
    let digit = remaining & 31;
    remaining >>>= 5;
    if (remaining > 0) {
      digit |= 32;
    }
    output += BASE64_VLQ[digit] ?? "";
  } while (remaining > 0);
  return output;
}

function containsVariant(value: unknown, variant: string): boolean {
  if (Array.isArray(value)) {
    return value.some((entry) => containsVariant(entry, variant));
  }
  if (!isRecord(value)) {
    return false;
  }
  if (Object.prototype.hasOwnProperty.call(value, variant)) {
    return true;
  }
  return Object.values(value).some((entry) => containsVariant(entry, variant));
}

function isResultReturnType(type: AstType | null | undefined): boolean {
  if (type === null || type === undefined) {
    return false;
  }
  if (variantOf(type.kind) !== "Path") {
    return false;
  }
  const path = payloadOf(type.kind) as TypePath;
  return path.segments[path.segments.length - 1] === "Result";
}

function stripUnderscores(value: string): string {
  return value.replaceAll("_", "");
}

function stripIntegerSuffix(value: string): string {
  const withoutSuffix = value.endsWith("u8") ? value.slice(0, -2) : value;
  return stripUnderscores(withoutSuffix);
}

function jsName(name: string): string {
  if (IDENTIFIER.test(name) && !JS_RESERVED.has(name)) {
    return name;
  }
  return `${name}$`;
}

function exportName(name: string): string {
  return IDENTIFIER.test(name) ? name : JSON.stringify(name);
}

function objectKey(name: string): string {
  return IDENTIFIER.test(name) ? name : JSON.stringify(name);
}

function memberAccess(name: string): string {
  return IDENTIFIER.test(name) ? `.${name}` : `[${JSON.stringify(name)}]`;
}

function helperName(name: HelperName): string {
  return `$${name}`;
}

function indent(text: string): string {
  return text
    .split("\n")
    .map((line) => (line.length === 0 ? line : `  ${line}`))
    .join("\n");
}
