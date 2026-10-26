export interface CodegenRequest {
  readonly moduleName: string;
  readonly ir: unknown;
}

export interface CodegenResult {
  readonly code: string;
  readonly sourceMap?: string;
}

export class CodegenNotImplementedError extends Error {
  readonly target = "ts";

  constructor(message = "The TypeScript backend is not implemented in the scaffold.") {
    super(message);
    this.name = "CodegenNotImplementedError";
  }
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function variantOf(value: unknown): string {
  if (typeof value === "string") {
    return value;
  }
  if (isRecord(value)) {
    const keys = Object.keys(value);
    if (keys.length === 1 && keys[0]) {
      return keys[0];
    }
  }
  throw new TypeError("Expected a serialized enum value.");
}

function payloadOf(value: unknown): unknown {
  if (typeof value === "string") {
    return undefined;
  }
  if (isRecord(value)) {
    const keys = Object.keys(value);
    if (keys.length === 1 && keys[0]) {
      return value[keys[0]];
    }
    return value;
  }
  return undefined;
}

function tsName(value: string): string {
  const normalized = value.replace(/[^A-Za-z0-9_$]/g, "_");
  if (/^[0-9]/.test(normalized)) {
    return `_${normalized}`;
  }
  return normalized;
}

function emitType(type: unknown): string {
  if (type === null || type === undefined) {
    return "unknown";
  }

  if (typeof type === "string") {
    switch (type) {
      case "I8":
      case "I16":
      case "I32":
      case "I64":
      case "U8":
      case "U16":
      case "U32":
      case "U64":
      case "Usize":
      case "Isize":
      case "F32":
      case "F64":
      case "i8":
      case "i16":
      case "i32":
      case "i64":
      case "u8":
      case "u16":
      case "u32":
      case "u64":
      case "usize":
      case "isize":
      case "f32":
      case "f64":
        return "number";
      case "Bool":
      case "bool":
        return "boolean";
      case "Str":
      case "str":
        return "string";
      case "Bytes":
      case "bytes":
        return "Uint8Array";
      case "Unit":
      case "unit":
        return "void";
      case "Never":
      case "never":
        return "never";
      case "Error":
      case "Unknown":
      case "unknown":
        return "unknown";
      default:
        return type;
    }
  }

  if (Array.isArray(type)) {
    return type.map((entry) => emitType(entry)).join(", ");
  }

  if (!isRecord(type)) {
    return "unknown";
  }

  if ("kind" in type && isRecord(type.kind)) {
    const kindTag = variantOf(type.kind);
    const kindPayload = payloadOf(type.kind) as Record<string, unknown> | undefined;
    switch (kindTag) {
      case "Path": {
        const segments = (kindPayload as Record<string, unknown> | undefined)?.segments as unknown[] | undefined;
        if (segments && segments.length > 0) {
          return emitType(String(segments[segments.length - 1]));
        }
        return "unknown";
      }
      case "Array":
      case "Slice": {
        return `${emitType((kindPayload as Record<string, unknown> | undefined)?.[kindTag] ?? "unknown")}[]`;
      }
      case "Map": {
        const map = (kindPayload as Record<string, unknown> | undefined)?.Map as unknown[] | undefined;
        if (Array.isArray(map)) {
          return `Map<${emitType(map[0] ?? "unknown")}, ${emitType(map[1] ?? "unknown")}>`;
        }
        return "Map<unknown, unknown>";
      }
      case "Tuple": {
        const tuple = (kindPayload as Record<string, unknown> | undefined)?.Tuple as unknown[] | undefined;
        return `[${(tuple ?? []).map((entry) => emitType(entry)).join(", ")}]`;
      }
      case "Function": {
        const fn = kindPayload as Record<string, unknown> | undefined;
        const parameters = ((fn?.parameters as unknown[]) ?? []).map((parameter) => emitType(parameter));
        const returnType = emitType(fn?.return_type ?? "void");
        return `(${parameters.join(", ")}) => ${returnType}`;
      }
      default:
        return "unknown";
    }
  }

  const tag = variantOf(type);
  switch (tag) {
    case "Option": {
      const inner = emitType((type as Record<string, unknown>).Option);
      return `${inner} | null`;
    }
    case "Result": {
      const result = (type as Record<string, unknown>).Result as unknown[];
      const okType = emitType(result[0] ?? "unknown");
      const errType = emitType(result[1] ?? "unknown");
      return `{ ok: true; value: ${okType} } | { ok: false; error: ${errType} }`;
    }
    case "Array":
    case "Slice": {
      return `${emitType((type as Record<string, unknown>)[tag])}[]`;
    }
    case "Tuple": {
      const tuple = (type as Record<string, unknown>).Tuple as unknown[];
      return `[${tuple.map((item) => emitType(item)).join(", ")}]`;
    }
    case "Map": {
      const mapEntry = (type as Record<string, unknown>).Map as unknown[];
      return `Map<${emitType(mapEntry[0] ?? "unknown")}, ${emitType(mapEntry[1] ?? "unknown")}>`;
    }
    case "FixedArray": {
      const fixed = (type as Record<string, unknown>).FixedArray as unknown[];
      return `${emitType(fixed[0] ?? "unknown")}[]`;
    }
    case "Function": {
      const fnType = (type as Record<string, unknown>).Function as Record<string, unknown>;
      const parameters = (fnType.parameters as unknown[] | undefined) ?? [];
      const params = parameters.map((parameter) => `${emitType(parameter)}`).join(", ");
      const returnType = emitType(fnType.return_type ?? "void");
      return `(${params}) => ${returnType}`;
    }
    case "Named": {
      const named = (type as Record<string, unknown>).Named as unknown[];
      const [name, args] = named as [string, unknown[]];
      const argsText = (args ?? []).map((arg) => emitType(arg)).join(", ");
      return argsText.length > 0 ? `${name}<${argsText}>` : name;
    }
    case "Generic": {
      return String((type as Record<string, unknown>).Generic ?? "unknown");
    }
    default:
      return "unknown";
  }
}

function emitFunctionSignature(item: Record<string, unknown>): string {
  const declaration = payloadOf(item.declaration) as Record<string, unknown> | undefined;
  const kind = item.kind;
  if (kind !== "Function" || !declaration) {
    return "";
  }

  const fn = declaration.Function ?? declaration;
  const name = String((fn as Record<string, unknown>).name ?? item.name ?? "unnamed");
  const params = ((fn as Record<string, unknown>).parameters as unknown[] | undefined) ?? [];
  const rendered = params
    .map((parameter) => {
      const param = parameter as Record<string, unknown>;
      const paramName = String(param.name ?? "arg");
      const paramType = emitType(param.ty ?? "unknown");
      return `${tsName(paramName)}: ${paramType}`;
    })
    .join(", ");

  const returnType = emitType((fn as Record<string, unknown>).return_type ?? "void");
  return `export function ${tsName(name)}(${rendered}): ${returnType} {\n  throw new Error("Not implemented: ${tsName(name)}");\n}`;
}

function emitStructType(item: Record<string, unknown>): string {
  const declaration = payloadOf(item.declaration) as Record<string, unknown> | undefined;
  const structName = String(item.name ?? "UnnamedStruct");
  const fields = (declaration?.fields as Record<string, unknown> | undefined) ?? {};
  const tag = variantOf(fields);

  if (tag === "Named") {
    const members = ((fields.Named as unknown[]) ?? [])
      .map((field) => {
        const entry = field as Record<string, unknown>;
        return `  ${tsName(String(entry.name))}: ${emitType(entry.ty ?? "unknown")};`;
      })
      .join("\n");
    return `export type ${tsName(structName)} = {\n${members}\n};`;
  }

  const tupleFields = ((fields.Tuple as unknown[]) ?? [])
    .map((entry) => emitType(entry ?? "unknown"))
    .join(", ");
  return `export type ${tsName(structName)} = [${tupleFields}];`;
}

function emitEnumType(item: Record<string, unknown>): string {
  const declaration = payloadOf(item.declaration) as Record<string, unknown> | undefined;
  const enumName = String(item.name ?? "UnnamedEnum");
  const variants = ((declaration?.variants as unknown[]) ?? [])
    .map((variant) => {
      const entry = variant as Record<string, unknown>;
      const variantName = String(entry.name ?? "variant");
      const fields = (entry.fields as unknown[] | undefined) ?? [];
      if (fields.length === 0) {
        return `  | { readonly $tag: "${variantName}" }`;
      }

      const fieldEntries = fields
        .map((field) => {
          const fieldValue = field as Record<string, unknown>;
          const fieldName = fieldValue.name ?? "value";
          return `    ${tsName(String(fieldName))}: ${emitType(fieldValue.ty ?? "unknown")}`;
        })
        .join("; ");
      return `  | { readonly $tag: "${variantName}"; ${fieldEntries} }`;
    })
    .join("\n");

  return `export type ${tsName(enumName)} =\n${variants};`;
}

function emitConstant(item: Record<string, unknown>): string {
  const declaration = payloadOf(item.declaration) as Record<string, unknown> | undefined;
  const name = String(item.name ?? "unnamed");
  const type = emitType((declaration?.ty ?? "unknown") as unknown);
  return `export const ${tsName(name)}: ${type} = undefined as any;`;
}

export function generateTypeScript(request: CodegenRequest): CodegenResult {
  const ir = request.ir as Record<string, unknown> | undefined;
  const hir = ir?.hir as Record<string, unknown> | undefined;
  if (!hir || !Array.isArray(hir.items)) {
    throw new CodegenNotImplementedError("The TypeScript backend requires a typed module with HIR items.");
  }

  const lines: string[] = [
    "// Generated by Neo Astra TypeScript backend.",
    "// This first TypeScript slice emits explicit TypeScript declarations and exports for the typed module.",
    "",
  ];

  for (const item of hir.items as unknown[]) {
    const entry = item as Record<string, unknown>;
    const kind = String(entry.kind ?? "");
    switch (kind) {
      case "Function":
        lines.push(emitFunctionSignature(entry));
        break;
      case "Struct":
        lines.push(emitStructType(entry));
        break;
      case "Enum":
        lines.push(emitEnumType(entry));
        break;
      case "Constant":
        lines.push(emitConstant(entry));
        break;
      default:
        break;
    }
    lines.push("");
  }

  const exports = hir.exports as Record<string, unknown> | undefined;
  if (exports && isRecord(exports.named)) {
    for (const [exportedName, exportedValue] of Object.entries(exports.named as Record<string, unknown>)) {
      const target = (exportedValue as Record<string, unknown>).target as Record<string, unknown> | undefined;
      const local = target && isRecord(target) ? target.Local : undefined;
      if (typeof local === "string") {
        const localName = tsName(local);
        const exportName = tsName(exportedName);
        lines.push(
          localName === exportName
            ? `export { ${localName} };`
            : `export { ${localName} as ${exportName} };`,
        );
      }
    }
  }

  if (exports && exports.default && isRecord(exports.default)) {
    const target = exports.default.target as Record<string, unknown> | undefined;
    const local = target && isRecord(target) ? target.Local : undefined;
    if (typeof local === "string") {
      lines.push(`export default ${tsName(local)};`);
    }
  }

  if (lines.at(-1) === "") {
    lines.pop();
  }

  return {
    code: lines.join("\n"),
  };
}
