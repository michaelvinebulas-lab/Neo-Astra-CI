import { existsSync, readFileSync } from "node:fs";
import { createRequire } from "node:module";
import { basename, join } from "node:path";
import { parse as parseToml } from "smol-toml";
import type { Taplo as TaploInstance } from "@taplo/lib";

export const MANIFEST_NAMES = ["core.json", "core.toml"] as const;

export type ManifestFormat = "json" | "toml";

export interface ManifestFile {
  readonly path: string;
  readonly name: (typeof MANIFEST_NAMES)[number];
  readonly format: ManifestFormat;
}

export interface ManifestDocument {
  readonly file: ManifestFile;
  readonly data: Record<string, unknown>;
}

export type ManifestErrorCode = "NS9001" | "NS9002";

export class ManifestError extends Error {
  readonly code: ManifestErrorCode;
  readonly path: string | undefined;

  constructor(code: ManifestErrorCode, message: string, path?: string) {
    super(message);
    this.name = "ManifestError";
    this.code = code;
    this.path = path;
  }
}

const require = createRequire(import.meta.url);
const taploModule = require("@taplo/lib") as {
  Taplo: { initialize: () => Promise<TaploInstance> };
};
let taploPromise: Promise<TaploInstance> | undefined;

const ROOT_ORDER = [
  "$schema",
  "package",
  "build",
  "nova",
  "registry",
  "math",
  "strings",
  "memory",
  "targets",
  "dependencies",
  "packageSources",
  "ffi",
  "capabilities",
] as const;

const KEY_ORDERS: Readonly<Record<string, readonly string[]>> = {
  package: [
    "name",
    "version",
    "edition",
    "license",
    "description",
    "authors",
    "repository",
    "homepage",
    "keywords",
  ],
  build: ["defaultTarget", "defaultMode", "profile", "outDir"],
  nova: ["entry", "watch"],
  registry: ["default", "defaultSource", "mirrors"],
  math: ["integers"],
  strings: ["model"],
  memory: ["default", "files"],
  target: ["backend", "module", "triple", "mode", "outDir", "wasi", "env"],
  dependency: [
    "source",
    "version",
    "path",
    "git",
    "ref",
    "rev",
    "tag",
    "targets",
    "features",
    "optional",
  ],
  packageSource: ["enabled", "lifecycleScripts", "registry", "index"],
  packageSources: ["path", "git", "npm", "cargo", "pypi", "nuget", "oci"],
  capability: ["web", "native", "wasm"],
  ffiTarget: ["source", "type", "ownership"],
};

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function sortedKeys(value: Record<string, unknown>): readonly string[] {
  return Object.keys(value).sort((left, right) => left.localeCompare(right));
}

function orderedKeys(
  value: Record<string, unknown>,
  order: readonly string[],
): readonly string[] {
  const present = order.filter((key) => Object.hasOwn(value, key));
  const remaining = sortedKeys(value).filter((key) => !order.includes(key));
  return [...present, ...remaining];
}

function orderRecord(
  value: Record<string, unknown>,
  order: readonly string[],
): Record<string, unknown> {
  const result: Record<string, unknown> = {};

  for (const key of orderedKeys(value, order)) {
    result[key] = orderValue(value[key], key);
  }

  return result;
}

function orderMap(
  value: Record<string, unknown>,
  order: readonly string[],
): Record<string, unknown> {
  const result: Record<string, unknown> = {};

  for (const key of sortedKeys(value)) {
    const entry = value[key];
    result[key] = isRecord(entry) ? orderRecord(entry, order) : orderValue(entry, key);
  }

  return result;
}

function orderKnownMap(
  value: Record<string, unknown>,
  outerOrder: readonly string[],
  childOrder: readonly string[],
): Record<string, unknown> {
  const result: Record<string, unknown> = {};

  for (const key of orderedKeys(value, outerOrder)) {
    const entry = value[key];
    result[key] = isRecord(entry)
      ? orderRecord(entry, childOrder)
      : orderValue(entry, key);
  }

  return result;
}

function orderFfi(value: Record<string, unknown>): Record<string, unknown> {
  const result: Record<string, unknown> = {};

  for (const moduleName of sortedKeys(value)) {
    const moduleValue = value[moduleName];
    if (!isRecord(moduleValue)) {
      result[moduleName] = orderValue(moduleValue, moduleName);
      continue;
    }

    const targets: Record<string, unknown> = {};
    for (const targetName of sortedKeys(moduleValue)) {
      const targetValue = moduleValue[targetName];
      targets[targetName] = isRecord(targetValue)
        ? orderRecord(targetValue, KEY_ORDERS["ffiTarget"] ?? [])
        : orderValue(targetValue, targetName);
    }
    result[moduleName] = targets;
  }

  return result;
}

function orderValue(value: unknown, key: string | undefined): unknown {
  if (Array.isArray(value)) {
    return value.map((entry) => orderValue(entry, undefined));
  }

  if (!isRecord(value)) {
    return value;
  }

  switch (key) {
    case undefined:
    case "$schema":
      return orderRecord(value, []);
    case "targets":
      return orderMap(value, KEY_ORDERS["target"] ?? []);
    case "dependencies":
      return orderMap(value, KEY_ORDERS["dependency"] ?? []);
    case "packageSources":
      return orderKnownMap(
        value,
        KEY_ORDERS["packageSources"] ?? [],
        KEY_ORDERS["packageSource"] ?? [],
      );
    case "ffi":
      return orderFfi(value);
    case "capabilities":
      return orderRecord(value, KEY_ORDERS["capability"] ?? []);
    case "memory":
    case "package":
    case "build":
    case "nova":
    case "registry":
    case "math":
    case "strings":
      return orderRecord(value, KEY_ORDERS[key] ?? []);
    default:
      return orderRecord(value, []);
  }
}

function orderManifest(data: Record<string, unknown>): Record<string, unknown> {
  return orderRecord(data, ROOT_ORDER);
}

function getTaplo(): Promise<TaploInstance> {
  taploPromise ??= taploModule.Taplo.initialize();
  return taploPromise;
}

export function findManifest(cwd = process.cwd()): ManifestFile {
  const matches = MANIFEST_NAMES.filter((name) => existsSync(join(cwd, name)));

  if (matches.length === 0) {
    throw new ManifestError(
      "NS9001",
      "No manifest found. Expected exactly one of core.json or core.toml.",
      cwd,
    );
  }

  if (matches.length > 1) {
    throw new ManifestError(
      "NS9001",
      "Both core.json and core.toml exist. Exactly one manifest is allowed.",
      cwd,
    );
  }

  const name = matches[0];
  if (name === undefined) {
    throw new ManifestError("NS9001", "Manifest discovery failed.", cwd);
  }

  return {
    path: join(cwd, name),
    name,
    format: name.endsWith(".json") ? "json" : "toml",
  };
}

export function manifestFileForPath(path: string): ManifestFile | undefined {
  const name = basename(path);
  if (name !== "core.json" && name !== "core.toml") {
    return undefined;
  }

  return {
    path,
    name,
    format: name.endsWith(".json") ? "json" : "toml",
  };
}

function assertNoDuplicateJsonKeys(source: string, file: ManifestFile): void {
  let index = 0;

  const skipWhitespace = (): void => {
    while (index < source.length && /\s/.test(source[index] ?? "")) {
      index += 1;
    }
  };

  const parseString = (): string => {
    const start = index;
    index += 1;

    while (index < source.length) {
      const character = source[index];
      if (character === "\\") {
        index += 2;
        continue;
      }
      if (character === '"') {
        index += 1;
        return JSON.parse(source.slice(start, index)) as string;
      }
      index += 1;
    }

    throw new ManifestError("NS9002", "Unterminated JSON string.", file.path);
  };

  const parseValue = (): void => {
    skipWhitespace();
    const character = source[index];

    if (character === "{") {
      parseObject();
      return;
    }

    if (character === "[") {
      parseArray();
      return;
    }

    if (character === '"') {
      parseString();
      return;
    }

    while (
      index < source.length &&
      !/[\s,\]}]/.test(source[index] ?? "")
    ) {
      index += 1;
    }
  };

  const parseObject = (): void => {
    index += 1;
    skipWhitespace();
    const keys = new Set<string>();

    if (source[index] === "}") {
      index += 1;
      return;
    }

    while (index < source.length) {
      skipWhitespace();
      if (source[index] !== '"') {
        throw new ManifestError("NS9002", "Invalid JSON object key.", file.path);
      }

      const key = parseString();
      if (keys.has(key)) {
        throw new ManifestError(
          "NS9002",
          `Duplicate key '${key}' in JSON manifest.`,
          file.path,
        );
      }
      keys.add(key);

      skipWhitespace();
      if (source[index] !== ":") {
        throw new ManifestError("NS9002", "Invalid JSON object entry.", file.path);
      }
      index += 1;
      parseValue();
      skipWhitespace();

      if (source[index] === ",") {
        index += 1;
        continue;
      }

      if (source[index] === "}") {
        index += 1;
        return;
      }

      throw new ManifestError("NS9002", "Invalid JSON object entry.", file.path);
    }

    throw new ManifestError("NS9002", "Unterminated JSON object.", file.path);
  };

  const parseArray = (): void => {
    index += 1;
    skipWhitespace();

    if (source[index] === "]") {
      index += 1;
      return;
    }

    while (index < source.length) {
      parseValue();
      skipWhitespace();

      if (source[index] === ",") {
        index += 1;
        continue;
      }

      if (source[index] === "]") {
        index += 1;
        return;
      }

      throw new ManifestError("NS9002", "Invalid JSON array entry.", file.path);
    }

    throw new ManifestError("NS9002", "Unterminated JSON array.", file.path);
  };

  parseValue();
  skipWhitespace();

  if (index !== source.length) {
    throw new ManifestError("NS9002", "Invalid JSON trailing content.", file.path);
  }
}

export function parseManifestSource(
  source: string,
  file: ManifestFile,
): Record<string, unknown> {
  let value: unknown;
  const normalizedSource = source.replace(/^\uFEFF/, "");

  try {
    if (file.format === "json") {
      value = JSON.parse(normalizedSource);
      assertNoDuplicateJsonKeys(normalizedSource, file);
    } else {
      value = parseToml(normalizedSource);
    }
  } catch (error) {
    if (error instanceof ManifestError) {
      throw error;
    }

    throw new ManifestError(
      "NS9002",
      `Invalid ${file.format.toUpperCase()} manifest: ${error instanceof Error ? error.message : String(error)}`,
      file.path,
    );
  }

  if (!isRecord(value)) {
    throw new ManifestError("NS9002", "Manifest root must be an object.", file.path);
  }

  return value;
}

export function readManifest(file: ManifestFile): Record<string, unknown> {
  let source: string;

  try {
    source = readFileSync(file.path, "utf8");
  } catch (error) {
    throw new ManifestError(
      "NS9002",
      `Unable to read manifest: ${error instanceof Error ? error.message : String(error)}`,
      file.path,
    );
  }

  return parseManifestSource(source, file);
}

export async function formatManifestSource(
  source: string,
  file: ManifestFile,
): Promise<string> {
  const data = parseManifestSource(source, file);

  if (file.format === "json") {
    return `${JSON.stringify(orderManifest(data), null, 2)}\n`;
  }

  const taplo = await getTaplo();
  let formatted: string;

  try {
    formatted = taplo.format(source.replace(/^\uFEFF/, ""));
  } catch (error) {
    throw new ManifestError(
      "NS9002",
      `Invalid TOML manifest: ${error instanceof Error ? error.message : String(error)}`,
      file.path,
    );
  }

  return `${formatted.replace(/\r\n/g, "\n").trimEnd()}\n`;
}

export function loadManifest(cwd = process.cwd()): ManifestDocument {
  const file = findManifest(cwd);
  return { file, data: readManifest(file) };
}

export function getPackageName(manifest: ManifestDocument): string | undefined {
  const pkg = manifest.data["package"];
  if (!isRecord(pkg)) {
    return undefined;
  }

  const name = pkg["name"];
  return typeof name === "string" ? name : undefined;
}
