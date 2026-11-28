/**
 * Browser-safe manifest parsing and validation.
 *
 * This is a strict subset of the CLI manifest surface: JSON and TOML parse to
 * the same object shape, wrong-manifest keys are rejected, and duplicate JSON
 * keys are detected instead of silently overwritten.
 */

export const CORE_MANIFEST_NAMES = ["core.json", "core.toml"] as const;
export const BUILD_MANIFEST_NAMES = ["build.json", "build.toml"] as const;

export const CORE_KEYS = [
  "package",
  "dependencies",
  "packageSources",
  "registry",
  "ffi",
  "capabilities",
  "$schema",
] as const;

export const BUILD_KEYS = [
  "build",
  "targets",
  "nova",
  "math",
  "strings",
  "memory",
  "toolchain",
] as const;

export type ManifestIssueCode = "NS9001" | "NS9002" | "NS9003";
export type ManifestFormat = "json" | "toml";
export type ManifestKind = "core" | "build";

export interface ManifestIssue {
  readonly code: ManifestIssueCode;
  readonly message: string;
  readonly file: string;
}

export interface ManifestDocument {
  readonly file: string;
  readonly format: ManifestFormat;
  readonly kind: ManifestKind;
  readonly data: Record<string, unknown>;
}

export interface ManifestValidation {
  readonly ok: boolean;
  readonly issues: readonly ManifestIssue[];
  readonly documents: readonly ManifestDocument[];
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function issue(code: ManifestIssueCode, message: string, file: string): ManifestIssue {
  return { code, message, file };
}

class JsonParser {
  readonly #text: string;
  #index = 0;

  constructor(text: string) {
    this.#text = text;
  }

  parse(): unknown {
    this.#skipWhitespace();
    const value = this.#parseValue();
    this.#skipWhitespace();
    if (this.#index !== this.#text.length) {
      throw new Error(`Unexpected trailing content at offset ${this.#index}.`);
    }
    return value;
  }

  #skipWhitespace(): void {
    while (this.#index < this.#text.length && /\s/.test(this.#text[this.#index] as string)) {
      this.#index++;
    }
  }

  #peek(): string | undefined {
    return this.#text[this.#index];
  }

  #parseValue(): unknown {
    const c = this.#peek();
    if (c === "{") return this.#parseObject();
    if (c === "[") return this.#parseArray();
    if (c === '"') return this.#parseString();
    if (c === "t") return this.#parseLiteral("true", true);
    if (c === "f") return this.#parseLiteral("false", false);
    if (c === "n") return this.#parseLiteral("null", null);

    const number = /^-?(?:0|[1-9]\d*)(?:\.\d+)?(?:[eE][+-]?\d+)?/.exec(this.#text.slice(this.#index));
    if (number !== null) {
      this.#index += number[0].length;
      return Number(number[0]);
    }
    throw new Error(`Unexpected character '${c ?? "EOF"}' at offset ${this.#index}.`);
  }

  #parseLiteral(word: string, value: unknown): unknown {
    if (!this.#text.startsWith(word, this.#index)) {
      throw new Error(`Invalid literal at offset ${this.#index}.`);
    }
    this.#index += word.length;
    return value;
  }

  #parseObject(): Record<string, unknown> {
    this.#index++;
    const result: Record<string, unknown> = Object.create(null) as Record<string, unknown>;
    this.#skipWhitespace();
    if (this.#peek() === "}") {
      this.#index++;
      return result;
    }
    for (;;) {
      this.#skipWhitespace();
      if (this.#peek() !== '"') {
        throw new Error(`Expected object key at offset ${this.#index}.`);
      }
      const key = this.#parseString();
      if (Object.hasOwn(result, key)) {
        throw new Error(`Duplicate key '${key}' at offset ${this.#index}.`);
      }
      this.#skipWhitespace();
      if (this.#peek() !== ":") {
        throw new Error(`Expected ':' after key '${key}'.`);
      }
      this.#index++;
      this.#skipWhitespace();
      result[key] = this.#parseValue();
      this.#skipWhitespace();
      const next = this.#peek();
      if (next === ",") {
        this.#index++;
        continue;
      }
      if (next === "}") {
        this.#index++;
        return result;
      }
      throw new Error(`Expected ',' or '}' at offset ${this.#index}.`);
    }
  }

  #parseArray(): unknown[] {
    this.#index++;
    const result: unknown[] = [];
    this.#skipWhitespace();
    if (this.#peek() === "]") {
      this.#index++;
      return result;
    }
    for (;;) {
      this.#skipWhitespace();
      result.push(this.#parseValue());
      this.#skipWhitespace();
      const next = this.#peek();
      if (next === ",") {
        this.#index++;
        continue;
      }
      if (next === "]") {
        this.#index++;
        return result;
      }
      throw new Error(`Expected ',' or ']' at offset ${this.#index}.`);
    }
  }

  #parseString(): string {
    this.#index++;
    let result = "";
    while (this.#index < this.#text.length) {
      const c = this.#text[this.#index] as string;
      this.#index++;
      if (c === '"') return result;
      if (c !== "\\") {
        result += c;
        continue;
      }
      const escape = this.#text[this.#index] as string;
      this.#index++;
      switch (escape) {
        case '"': result += '"'; break;
        case "\\": result += "\\"; break;
        case "/": result += "/"; break;
        case "b": result += "\b"; break;
        case "f": result += "\f"; break;
        case "n": result += "\n"; break;
        case "r": result += "\r"; break;
        case "t": result += "\t"; break;
        case "u": {
          const hex = this.#text.slice(this.#index, this.#index + 4);
          if (!/^[0-9a-fA-F]{4}$/.test(hex)) {
            throw new Error(`Invalid unicode escape at offset ${this.#index}.`);
          }
          result += String.fromCharCode(Number.parseInt(hex, 16));
          this.#index += 4;
          break;
        }
        default:
          throw new Error(`Invalid escape '\\${escape}'.`);
      }
    }
    throw new Error("Unterminated string.");
  }
}

function parseJson(text: string): Record<string, unknown> {
  const value = new JsonParser(text).parse();
  if (!isRecord(value)) {
    throw new Error("Manifest root must be an object.");
  }
  return value;
}

function splitTomlArray(body: string): string[] {
  const parts: string[] = [];
  let current = "";
  let quote: string | undefined;
  let depth = 0;
  for (const c of body) {
    if (quote !== undefined) {
      current += c;
      if (c === "\\" && quote === '"') {
        continue;
      }
      if (c === quote) quote = undefined;
      continue;
    }
    if (c === '"' || c === "'") {
      quote = c;
      current += c;
      continue;
    }
    if (c === "[" || c === "{") {
      depth++;
      current += c;
      continue;
    }
    if (c === "]" || c === "}") {
      depth--;
      current += c;
      continue;
    }
    if (c === "," && depth === 0) {
      parts.push(current.trim());
      current = "";
      continue;
    }
    current += c;
  }
  if (quote !== undefined || depth !== 0) {
    throw new Error("Unterminated TOML array.");
  }
  if (current.trim() !== "") parts.push(current.trim());
  return parts;
}

function parseTomlValue(raw: string): unknown {
  const text = raw.trim();
  if (text === "") throw new Error("Missing TOML value.");
  if (text.startsWith('"""') && text.endsWith('"""') && text.length >= 6) {
    return text.slice(3, -3);
  }
  if (text.startsWith('"') && text.endsWith('"') && text.length >= 2) {
    return JSON.parse(text) as string;
  }
  if (text.startsWith("'") && text.endsWith("'") && text.length >= 2) {
    return text.slice(1, -1);
  }
  if (text === "true") return true;
  if (text === "false") return false;
  if (text.startsWith("[") && text.endsWith("]")) {
    return splitTomlArray(text.slice(1, -1)).map((part) => parseTomlValue(part));
  }
  if (/^-?\d+$/.test(text)) return Number.parseInt(text, 10);
  if (/^-?\d+\.\d+$/.test(text)) return Number.parseFloat(text);
  throw new Error(`Unsupported TOML value '${text}'.`);
}

function stripTomlComment(line: string): string {
  let quote: string | undefined;
  for (let i = 0; i < line.length; i++) {
    const c = line[i] as string;
    if (quote !== undefined) {
      if (c === "\\" && quote === '"') {
        i++;
        continue;
      }
      if (c === quote) quote = undefined;
      continue;
    }
    if (c === '"' || c === "'") {
      quote = c;
      continue;
    }
    if (c === "#") return line.slice(0, i);
  }
  return line;
}

function setTablePath(root: Record<string, unknown>, path: string[]): Record<string, unknown> {
  let current = root;
  for (const segment of path) {
    const existing = current[segment];
    if (existing === undefined) {
      const next: Record<string, unknown> = Object.create(null) as Record<string, unknown>;
      current[segment] = next;
      current = next;
      continue;
    }
    if (!isRecord(existing)) {
      throw new Error(`Conflicting TOML table '${segment}'.`);
    }
    current = existing;
  }
  return current;
}

function parseToml(text: string): Record<string, unknown> {
  const root: Record<string, unknown> = Object.create(null) as Record<string, unknown>;
  let current = root;
  const lines = stripTomlComment(text).split(/\r?\n/);

  for (let lineNumber = 0; lineNumber < lines.length; lineNumber++) {
    const line = (lines[lineNumber] as string).trim();
    if (line === "") continue;
    if (line.startsWith("[") && line.endsWith("]") && !line.startsWith("[[")) {
      const name = line.slice(1, -1).trim();
      if (name === "") throw new Error(`Empty TOML table on line ${lineNumber + 1}.`);
      const path = name.split(".").map((part) => part.trim());
      if (path.some((part) => part === "")) {
        throw new Error(`Invalid TOML table '${name}' on line ${lineNumber + 1}.`);
      }
      current = setTablePath(root, path);
      continue;
    }

    const equals = line.indexOf("=");
    if (equals < 0) {
      throw new Error(`Expected '=' on line ${lineNumber + 1}.`);
    }
    const key = line.slice(0, equals).trim();
    if (key === "") throw new Error(`Empty key on line ${lineNumber + 1}.`);
    if (Object.hasOwn(current, key)) {
      throw new Error(`Duplicate TOML key '${key}' on line ${lineNumber + 1}.`);
    }
    current[key] = parseTomlValue(line.slice(equals + 1));
  }

  return root;
}

function manifestKind(name: string): ManifestKind | undefined {
  if ((CORE_MANIFEST_NAMES as readonly string[]).includes(name)) return "core";
  if ((BUILD_MANIFEST_NAMES as readonly string[]).includes(name)) return "build";
  return undefined;
}

function manifestFormat(name: string): ManifestFormat {
  return name.endsWith(".json") ? "json" : "toml";
}

export function parseManifestSource(source: string, file: string): Record<string, unknown> {
  const format = manifestFormat(file);
  try {
    return format === "json" ? parseJson(source) : parseToml(source);
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    throw new Error(`Invalid ${format.toUpperCase()} in ${file}: ${message}`);
  }
}

export function validateManifestFiles(files: ReadonlyMap<string, string>): ManifestValidation {
  const issues: ManifestIssue[] = [];
  const documents: ManifestDocument[] = [];

  const present = [...files.keys()]
    .filter((name) => manifestKind(name) !== undefined)
    .sort((a, b) => a.localeCompare(b));
  const coreFiles = present.filter((name) => manifestKind(name) === "core");
  const buildFiles = present.filter((name) => manifestKind(name) === "build");

  if (coreFiles.length === 0) {
    issues.push(issue("NS9001", "Missing core.json or core.toml.", "core.json"));
  } else if (coreFiles.length > 1) {
    issues.push(
      issue("NS9002", "Exactly one of core.json or core.toml must exist.", coreFiles[1] as string),
    );
  }
  if (buildFiles.length > 1) {
    issues.push(
      issue("NS9002", "Exactly one of build.json or build.toml may exist.", buildFiles[1] as string),
    );
  }

  for (const file of present) {
    const text = files.get(file) as string;
    let data: Record<string, unknown>;
    try {
      data = parseManifestSource(text, file);
    } catch (error) {
      issues.push(
        issue("NS9002", error instanceof Error ? error.message : String(error), file),
      );
      continue;
    }

    const kind = manifestKind(file) as ManifestKind;
    const allowed = new Set<string>(kind === "core" ? CORE_KEYS : BUILD_KEYS);
    const forbidden = new Set<string>(kind === "core" ? BUILD_KEYS : CORE_KEYS);

    for (const key of Object.keys(data)) {
      if (forbidden.has(key)) {
        const other = kind === "core" ? "build" : "core";
        const ext = file.endsWith(".json") ? "json" : "toml";
        issues.push(
          issue("NS9003", `Key '${key}' belongs in ${other}.${ext}, not ${file}.`, file),
        );
      } else if (!allowed.has(key)) {
        issues.push(issue("NS9002", `Unknown key '${key}' in ${file}.`, file));
      }
    }

    documents.push({ file, format: manifestFormat(file), kind, data });
  }

  const lockText = files.get("core.lock.json");
  if (lockText !== undefined) {
    try {
      const lock = parseJson(lockText);
      if (!Array.isArray(lock["packages"])) {
        issues.push(issue("NS9002", "core.lock.json must contain a 'packages' array.", "core.lock.json"));
      }
    } catch (error) {
      issues.push(
        issue(
          "NS9002",
          error instanceof Error ? error.message : String(error),
          "core.lock.json",
        ),
      );
    }
  }

  return { ok: issues.length === 0, issues, documents };
}
