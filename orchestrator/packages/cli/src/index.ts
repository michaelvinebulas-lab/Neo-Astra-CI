#!/usr/bin/env node

import { existsSync, mkdirSync, readFileSync, readdirSync, statSync, writeFileSync } from "node:fs";
import {
  basename,
  dirname,
  extname,
  isAbsolute,
  join,
  relative,
  resolve,
  sep,
} from "node:path";
import { TextDecoder } from "node:util";
import { pathToFileURL } from "node:url";
import { generateJavaScript } from "@neo-astra/codegen-js";
import {
  CompilerClient,
  CompilerClientError,
  type CompilerDiagnostic,
  type Mode,
} from "@neo-astra/compiler-client";
import {
  ManifestError,
  formatManifestSource,
  getPackageName,
  loadManifest,
  manifestFileForPath,
  type ManifestDocument,
} from "@neo-astra/manifest";

const VERSION = "0.1.0";
const SOURCE_EXTENSION = ".ns";

const IGNORED_DIRECTORIES = new Set([
  "build",
  "coverage",
  "dist",
  "node_modules",
  "out",
  "target",
  ".astra",
  ".turbo",
  ".next",
]);

const HELP = `Neo Astra compiler

Usage:
  astra <command> [options]

Commands:
  check [dir] [--json]   Discover and validate the project manifest
  build [dir]            Build the configured target
  fmt [path] [--check]   Format .ns source and core.json/core.toml
  ast [dir]              Print the canonical AST (not implemented yet)
  explain [dir]          Explain mode/capability inference (not implemented yet)

Options:
  -h, --help             Show this help
  -v, --version          Show the CLI version

build options:
  --target <name>        Select a target from the manifest
  --compiler <path>      Use a specific astra-compiler binary
  --json                 Print a machine-readable build report

fmt options:
  --check                Report formatting differences without writing
  --json                 Print a machine-readable report
  --compiler <path>      Use a specific astra-compiler binary

Exit codes:
  0  success
  1  invalid input, project error, or build/codegen failure
  2  command not implemented
`;

type CliErrorCode =
  | "NS8001"
  | "NS8003"
  | "NS9000"
  | "NS9001"
  | "NS9007"
  | "NS9008"
  | "NV0002";

type FormatFileKind = "source" | "manifest";

interface SourceFile {
  readonly path: string;
  readonly displayPath: string;
  readonly kind: FormatFileKind;
}

interface ReportedDiagnostic {
  readonly code: string;
  readonly severity: CompilerDiagnostic["severity"];
  readonly message: string;
  readonly hint: string | null;
  readonly path: string | null;
  readonly line: number | null;
  readonly column: number | null;
}

type FileStatus = "formatted" | "unchanged" | "would-reformat" | "error";

interface FileResult {
  readonly file: SourceFile;
  readonly status: FileStatus;
  readonly formatted: string | null;
  readonly diagnostics: readonly ReportedDiagnostic[];
}

interface FmtOptions {
  readonly path: string;
  readonly check: boolean;
  readonly json: boolean;
  readonly compilerPath: string | undefined;
}

interface BuildOptions {
  readonly project: string;
  readonly target: string | undefined;
  readonly json: boolean;
  readonly compilerPath: string | undefined;
}

interface BuildReport {
  readonly target: string;
  readonly backend: string;
  readonly entry: string;
  readonly output: string | null;
  readonly diagnostics: readonly ReportedDiagnostic[];
}

class CliError extends Error {
  readonly code: CliErrorCode;
  readonly path: string | undefined;

  constructor(code: CliErrorCode, message: string, path?: string) {
    super(message);
    this.name = "CliError";
    this.code = code;
    this.path = path;
  }
}

function printHelp(): void {
  process.stdout.write(HELP);
}

function printVersion(): void {
  process.stdout.write(`astra ${VERSION}\n`);
}

function manifestJson(manifest: ManifestDocument): string {
  return JSON.stringify(
    {
      manifest: {
        path: manifest.file.path,
        name: manifest.file.name,
        format: manifest.file.format,
      },
      package: getPackageName(manifest) ?? null,
    },
    null,
    2,
  );
}

function manifestText(manifest: ManifestDocument): string {
  const packageName = getPackageName(manifest);
  const lines = [
    `Manifest: ${manifest.file.path}`,
    `Format: ${manifest.file.format}`,
  ];

  if (packageName !== undefined) {
    lines.push(`Package: ${packageName}`);
  }

  return `${lines.join("\n")}\n`;
}

function diagnosticJson(error: ManifestError): string {
  return JSON.stringify(
    {
      code: error.code,
      severity: "error",
      message: error.message,
      ...(error.path === undefined ? {} : { path: error.path }),
    },
    null,
    2,
  );
}

function isJsonFlag(args: readonly string[]): boolean {
  return args.includes("--json");
}

function commandDirectory(args: readonly string[], cwd: string): string {
  const candidate = args.find((arg) => !arg.startsWith("-"));
  return candidate ?? cwd;
}

function runCheck(args: readonly string[], cwd: string): number {
  const json = isJsonFlag(args);

  try {
    const manifest = loadManifest(commandDirectory(args, cwd));
    process.stdout.write(json ? `${manifestJson(manifest)}\n` : manifestText(manifest));
    return 0;
  } catch (error) {
    if (error instanceof ManifestError) {
      process.stderr.write(json ? `${diagnosticJson(error)}\n` : `${error.code}: ${error.message}\n`);
      return 1;
    }

    const message = error instanceof Error ? error.message : String(error);
    process.stderr.write(`NS9000: ${message}\n`);
    return 1;
  }
}

function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

function displayPath(path: string, cwd: string): string {
  const relativePath = relative(cwd, path);

  if (
    relativePath.length === 0 ||
    relativePath === ".." ||
    relativePath.startsWith(`..${sep}`)
  ) {
    return path.split(sep).join("/");
  }

  return relativePath.split(sep).join("/");
}

function parseFmtArgs(args: readonly string[], cwd: string): FmtOptions {
  let check = false;
  let json = false;
  let compilerPath: string | undefined;
  const positional: string[] = [];

  for (let index = 0; index < args.length; index += 1) {
    const arg = args[index];

    if (arg === undefined) {
      continue;
    }

    if (arg === "--") {
      positional.push(...args.slice(index + 1));
      break;
    }

    if (arg === "--check") {
      check = true;
      continue;
    }

    if (arg === "--json") {
      json = true;
      continue;
    }

    if (arg === "--compiler") {
      const value = args[index + 1];
      if (value === undefined || value.startsWith("-")) {
        throw new CliError("NS9000", "The --compiler option requires a binary path.");
      }

      compilerPath = value;
      index += 1;
      continue;
    }

    if (arg.startsWith("-")) {
      throw new CliError("NS9000", `Unknown fmt option '${arg}'.`);
    }

    positional.push(arg);
  }

  if (positional.length > 1) {
    throw new CliError("NS9000", "astra fmt accepts at most one file or directory.");
  }

  return {
    path: resolve(cwd, positional[0] ?? "."),
    check,
    json,
    compilerPath,
  };
}

function parseBuildArgs(args: readonly string[], cwd: string): BuildOptions {
  let json = false;
  let target: string | undefined;
  let compilerPath: string | undefined;
  const positional: string[] = [];

  for (let index = 0; index < args.length; index += 1) {
    const arg = args[index];
    if (arg === undefined) {
      continue;
    }

    if (arg === "--") {
      positional.push(...args.slice(index + 1));
      break;
    }

    if (arg === "--json") {
      json = true;
      continue;
    }

    if (arg === "--target" || arg === "--compiler") {
      const value = args[index + 1];
      if (value === undefined || value.startsWith("-")) {
        throw new CliError("NS9000", `The ${arg} option requires a value.`);
      }
      if (arg === "--target") {
        target = value;
      } else {
        compilerPath = value;
      }
      index += 1;
      continue;
    }

    if (arg.startsWith("-")) {
      throw new CliError("NS9000", `Unknown build option '${arg}'.`);
    }

    positional.push(arg);
  }

  if (positional.length > 1) {
    throw new CliError("NS9000", "astra build accepts at most one project directory.");
  }

  return {
    project: resolve(cwd, positional[0] ?? "."),
    target,
    json,
    compilerPath,
  };
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function isMode(value: unknown): value is Mode {
  return value === "core" || value === "app" || value === "web" || value === "system";
}

function isIntegerPolicy(
  value: unknown,
): value is "wrapping" | "safe-checked" | "arbitrary" {
  return value === "wrapping" || value === "safe-checked" || value === "arbitrary";
}

function collectSourceFiles(inputPath: string, cwd: string): readonly SourceFile[] {
  let inputStat;

  try {
    inputStat = statSync(inputPath);
  } catch (error) {
    throw new CliError(
      "NS9008",
      `Unable to access the path: ${errorMessage(error)}`,
      displayPath(inputPath, cwd),
    );
  }

  if (inputStat.isFile()) {
    if (manifestFileForPath(inputPath) !== undefined) {
      return [
        {
          path: inputPath,
          displayPath: displayPath(inputPath, cwd),
          kind: "manifest",
        },
      ];
    }

    if (extname(inputPath).toLowerCase() !== SOURCE_EXTENSION) {
      throw new CliError(
        "NS9008",
        "The target is not a .ns source file or a core.json/core.toml manifest.",
        displayPath(inputPath, cwd),
      );
    }

    return [
      {
        path: inputPath,
        displayPath: displayPath(inputPath, cwd),
        kind: "source",
      },
    ];
  }

  if (!inputStat.isDirectory()) {
    throw new CliError(
      "NS9008",
      "The target is not a file or directory.",
      displayPath(inputPath, cwd),
    );
  }

  const files: SourceFile[] = [];
  const manifests = ["core.json", "core.toml"].filter((name) =>
    existsSync(join(inputPath, name)),
  );

  if (manifests.length > 1) {
    throw new CliError(
      "NS9001",
      "Both core.json and core.toml exist. Exactly one manifest is allowed.",
      displayPath(inputPath, cwd),
    );
  }

  const manifestName = manifests[0];
  if (manifestName !== undefined) {
    const manifestPath = join(inputPath, manifestName);
    files.push({
      path: manifestPath,
      displayPath: displayPath(manifestPath, cwd),
      kind: "manifest",
    });
  }

  const walk = (directory: string): void => {
    let entries;

    try {
      entries = readdirSync(directory, { withFileTypes: true });
    } catch (error) {
      throw new CliError(
        "NS9008",
        `Unable to read the directory: ${errorMessage(error)}`,
        displayPath(directory, cwd),
      );
    }

    entries.sort((left, right) => left.name.localeCompare(right.name));

    for (const entry of entries) {
      if (entry.isDirectory()) {
        const name = entry.name.toLowerCase();
        if (!entry.name.startsWith(".") && !IGNORED_DIRECTORIES.has(name)) {
          walk(join(directory, entry.name));
        }
        continue;
      }

      if (entry.isFile() && extname(entry.name).toLowerCase() === SOURCE_EXTENSION) {
        files.push({
          path: join(directory, entry.name),
          displayPath: displayPath(join(directory, entry.name), cwd),
          kind: "source",
        });
      }
    }
  };

  walk(inputPath);

  return files.sort((left, right) =>
    left.displayPath.localeCompare(right.displayPath),
  );
}

function lineColumn(
  source: string,
  byteOffset: number,
): { line: number; column: number } {
  const bytes = Buffer.from(source, "utf8");
  const end = Math.max(0, Math.min(byteOffset, bytes.length));
  const prefix = bytes.subarray(0, end).toString("utf8");
  const lines = prefix.split("\n");
  const last = lines[lines.length - 1] ?? "";

  return { line: lines.length, column: [...last].length + 1 };
}

function reportDiagnostic(
  file: SourceFile,
  source: string | null,
  diagnostic: CompilerDiagnostic,
): ReportedDiagnostic {
  const position =
    source !== null && diagnostic.span !== null
      ? lineColumn(source, diagnostic.span.start)
      : { line: null, column: null };

  return {
    code: diagnostic.code,
    severity: diagnostic.severity,
    message: diagnostic.message,
    hint: diagnostic.hint,
    path: file.displayPath,
    line: position.line,
    column: position.column,
  };
}

function errorDiagnostic(error: unknown, path: string | null = null): ReportedDiagnostic {
  if (error instanceof CliError) {
    return {
      code: error.code,
      severity: "error",
      message: error.message,
      hint: null,
      path: error.path ?? path,
      line: null,
      column: null,
    };
  }

  if (error instanceof ManifestError) {
    return {
      code: error.code,
      severity: "error",
      message: error.message,
      hint: null,
      path: error.path ?? path,
      line: null,
      column: null,
    };
  }

  if (error instanceof CompilerClientError) {
    return {
      code: error.code,
      severity: "error",
      message: error.message,
      hint: null,
      path,
      line: null,
      column: null,
    };
  }

  return {
    code: "NS9000",
    severity: "error",
    message: errorMessage(error),
    hint: null,
    path,
    line: null,
    column: null,
  };
}

function printDiagnostic(diagnostic: ReportedDiagnostic): void {
  const location =
    diagnostic.path === null
      ? null
      : diagnostic.line === null
        ? diagnostic.path
        : `${diagnostic.path}:${diagnostic.line}:${diagnostic.column ?? 1}`;
  const prefix = location === null ? "" : `${location}: `;
  const help = diagnostic.hint === null ? "" : `\n  help: ${diagnostic.hint}`;

  process.stderr.write(
    `${prefix}${diagnostic.severity}[${diagnostic.code}]: ${diagnostic.message}${help}\n`,
  );
}

function reportJson(
  check: boolean,
  results: readonly FileResult[],
  diagnostics: readonly ReportedDiagnostic[],
): void {
  const hasErrors = results.some((result) => result.status === "error");
  const hasChanges = results.some((result) => result.status === "would-reformat");

  process.stdout.write(
    `${JSON.stringify(
      {
        check,
        ok: !hasErrors && (!check || !hasChanges),
        files: results.map((result) => ({
          path: result.file.displayPath,
          status: result.status,
          diagnostics: result.diagnostics,
        })),
        diagnostics,
      },
      null,
      2,
    )}\n`,
  );
}

function emitFatal(error: unknown, check: boolean, json: boolean): number {
  const diagnostic = errorDiagnostic(error);
  if (json) {
    reportJson(check, [], [diagnostic]);
  } else {
    printDiagnostic(diagnostic);
  }

  return 1;
}

function readSource(file: SourceFile): string {
  let bytes: Buffer;

  try {
    bytes = readFileSync(file.path);
  } catch (error) {
    throw new CliError(
      "NS9008",
      `Unable to read the file: ${errorMessage(error)}`,
      file.displayPath,
    );
  }

  try {
    return new TextDecoder("utf-8", { fatal: true, ignoreBOM: true }).decode(bytes);
  } catch {
    throw new CliError(
      "NS9007",
      "Source is not valid UTF-8.",
      file.displayPath,
    );
  }
}

async function runFmt(args: readonly string[], cwd: string): Promise<number> {
  if (args.includes("-h") || args.includes("--help")) {
    printHelp();
    return 0;
  }

  let options: FmtOptions;
  try {
    options = parseFmtArgs(args, cwd);
  } catch (error) {
    return emitFatal(error, false, isJsonFlag(args));
  }

  let files: readonly SourceFile[];
  try {
    files = collectSourceFiles(options.path, cwd);
  } catch (error) {
    return emitFatal(error, options.check, options.json);
  }

  if (files.length === 0) {
    if (options.json) {
      reportJson(options.check, [], []);
    } else {
      process.stdout.write("No formattable files found.\n");
    }
    return 0;
  }

  let client: CompilerClient | undefined;
  const getClient = (): CompilerClient => {
    client ??= new CompilerClient({
      cwd,
      ...(options.compilerPath === undefined
        ? {}
        : { binaryPath: options.compilerPath }),
    });
    return client;
  };

  const results: FileResult[] = [];
  const allDiagnostics: ReportedDiagnostic[] = [];

  for (const file of files) {
    let source: string;
    try {
      source = readSource(file);
    } catch (error) {
      const diagnostic = errorDiagnostic(error, file.displayPath);
      allDiagnostics.push(diagnostic);
      results.push({
        file,
        status: "error",
        formatted: null,
        diagnostics: [diagnostic],
      });
      continue;
    }

    if (file.kind === "manifest") {
      const manifestFile = manifestFileForPath(file.path);
      if (manifestFile === undefined) {
        const diagnostic: ReportedDiagnostic = {
          code: "NS9002",
          severity: "error",
          message: "Unsupported manifest file name.",
          hint: null,
          path: file.displayPath,
          line: null,
          column: null,
        };
        allDiagnostics.push(diagnostic);
        results.push({
          file,
          status: "error",
          formatted: null,
          diagnostics: [diagnostic],
        });
        continue;
      }

      let formatted: string;
      try {
        formatted = await formatManifestSource(source, manifestFile);
      } catch (error) {
        const diagnostic = errorDiagnostic(error, file.displayPath);
        allDiagnostics.push(diagnostic);
        results.push({
          file,
          status: "error",
          formatted: null,
          diagnostics: [diagnostic],
        });
        continue;
      }

      const changed = formatted !== source;
      results.push({
        file,
        status: changed
          ? options.check
            ? "would-reformat"
            : "formatted"
          : "unchanged",
        formatted: changed ? formatted : null,
        diagnostics: [],
      });
      continue;
    }

    let response;
    try {
      response = getClient().formatSource(file.displayPath, source);
    } catch (error) {
      const diagnostic = errorDiagnostic(error, file.displayPath);
      allDiagnostics.push(diagnostic);
      results.push({
        file,
        status: "error",
        formatted: null,
        diagnostics: [diagnostic],
      });
      break;
    }

    const fileDiagnostics = response.diagnostics.map((diagnostic) =>
      reportDiagnostic(file, source, diagnostic),
    );
    allDiagnostics.push(...fileDiagnostics);

    if (
      response.source === null &&
      !fileDiagnostics.some((diagnostic) => diagnostic.severity === "error")
    ) {
      const diagnostic: ReportedDiagnostic = {
        code: "NS9005",
        severity: "error",
        message: "Compiler returned no formatted source.",
        hint: null,
        path: file.displayPath,
        line: null,
        column: null,
      };
      fileDiagnostics.push(diagnostic);
      allDiagnostics.push(diagnostic);
    }

    if (
      response.source === null ||
      fileDiagnostics.some((diagnostic) => diagnostic.severity === "error")
    ) {
      results.push({
        file,
        status: "error",
        formatted: null,
        diagnostics: fileDiagnostics,
      });
      continue;
    }

    const changed = response.source !== source;
    const status: FileStatus = changed
      ? options.check
        ? "would-reformat"
        : "formatted"
      : "unchanged";
    results.push({
      file,
      status,
      formatted: changed ? response.source : null,
      diagnostics: fileDiagnostics,
    });
  }

  if (results.some((result) => result.status === "error")) {
    if (options.json) {
      reportJson(options.check, results, allDiagnostics);
    } else {
      for (const diagnostic of allDiagnostics) {
        printDiagnostic(diagnostic);
      }
    }
    return 1;
  }

  if (!options.json) {
    for (const diagnostic of allDiagnostics) {
      printDiagnostic(diagnostic);
    }
  }

  if (options.check) {
    if (options.json) {
      reportJson(options.check, results, allDiagnostics);
    } else {
      const changed = results.filter((result) => result.status === "would-reformat");
      for (const result of changed) {
        process.stdout.write(`would reformat ${result.file.displayPath}\n`);
      }

      process.stdout.write(
        changed.length === 0
          ? `All ${results.length} file(s) are formatted.\n`
          : `${changed.length} file(s) need formatting.\n`,
      );
    }

    return results.some((result) => result.status === "would-reformat") ? 1 : 0;
  }

  const changed = results.filter((result) => result.status === "formatted");
  for (const result of changed) {
    if (result.formatted === null) {
      continue;
    }

    try {
      writeFileSync(result.file.path, result.formatted, "utf8");
    } catch (error) {
      return emitFatal(
        new CliError(
          "NS9008",
          `Unable to write the file: ${errorMessage(error)}`,
          result.file.displayPath,
        ),
        options.check,
        options.json,
      );
    }
  }

  if (options.json) {
    reportJson(options.check, results, allDiagnostics);
  } else {
    for (const result of changed) {
      process.stdout.write(`formatted ${result.file.displayPath}\n`);
    }

    process.stdout.write(
      changed.length === 0
        ? `All ${results.length} file(s) already formatted.\n`
        : `${changed.length} file(s) formatted.\n`,
    );
  }

  return 0;
}

function printBuildReport(report: BuildReport): void {
  process.stdout.write(`${JSON.stringify(report, null, 2)}\n`);
}

function codegenDiagnostic(
  file: SourceFile,
  source: string,
  diagnostic: { readonly code: string; readonly message: string; readonly span?: { readonly start: number } },
): ReportedDiagnostic {
  const position =
    diagnostic.span === undefined
      ? { line: null, column: null }
      : lineColumn(source, diagnostic.span.start);

  return {
    code: diagnostic.code,
    severity: "error",
    message: diagnostic.message,
    hint: null,
    path: file.displayPath,
    line: position.line,
    column: position.column,
  };
}

interface ProjectModuleSource {
  readonly key: string;
  readonly absolutePath: string;
  readonly text: string;
}

function scanImportSpecifiers(source: string): readonly string[] {
  const specifiers = new Set<string>();
  const fromPattern = /^[ \t]*(?:import|export)\b[^\n;]*?\bfrom\s*"([^"]+)"/gm;
  const sideEffectPattern = /^[ \t]*import\s*"([^"]+)"/gm;
  const pathPattern =
    /^[ \t]*import\s+([A-Za-z_][A-Za-z0-9_]*(?:\.[A-Za-z_][A-Za-z0-9_]*)+)[ \t]*(?:;)?$/gm;
  for (const pattern of [fromPattern, sideEffectPattern]) {
    for (const match of source.matchAll(pattern)) {
      if (match[1] !== undefined) {
        specifiers.add(match[1]);
      }
    }
  }
  for (const match of source.matchAll(pathPattern)) {
    if (match[1] !== undefined) {
      specifiers.add(match[1]);
    }
  }
  return [...specifiers];
}

function resolveProjectSpecifier(
  importerKey: string,
  specifier: string,
  root: string,
): string | undefined {
  if (specifier.startsWith("./") || specifier.startsWith("../")) {
    let target = resolve(dirname(join(root, importerKey)), specifier);
    if (!target.toLowerCase().endsWith(SOURCE_EXTENSION)) {
      target = `${target}${SOURCE_EXTENSION}`;
    }
    const relativeTarget = relative(root, target);
    if (
      relativeTarget.length === 0 ||
      isAbsolute(relativeTarget) ||
      relativeTarget === ".." ||
      relativeTarget.startsWith(`..${sep}`)
    ) {
      return undefined;
    }
    return relativeTarget.split(sep).join("/");
  }

  if (
    !specifier.includes("/") &&
    specifier.includes(".") &&
    !specifier.toLowerCase().endsWith(SOURCE_EXTENSION)
  ) {
    return `${specifier.replaceAll(".", "/")}${SOURCE_EXTENSION}`;
  }

  return undefined;
}

function discoverProjectModules(
  root: string,
  entryPath: string,
  entryKey: string,
  cwd: string,
): readonly ProjectModuleSource[] {
  const ordered: ProjectModuleSource[] = [];
  const visited = new Set<string>();

  const visit = (key: string, absolutePath: string): void => {
    if (visited.has(key)) {
      return;
    }
    visited.add(key);
    const file: SourceFile = {
      path: absolutePath,
      displayPath: displayPath(absolutePath, cwd),
      kind: "source",
    };
    const text = readSource(file);
    for (const specifier of scanImportSpecifiers(text)) {
      const targetKey = resolveProjectSpecifier(key, specifier, root);
      if (targetKey === undefined || visited.has(targetKey)) {
        continue;
      }
      const targetPath = resolve(root, targetKey);
      if (!existsSync(targetPath)) {
        continue;
      }
      visit(targetKey, targetPath);
    }
    ordered.push({ key, absolutePath, text });
  };

  visit(entryKey, entryPath);
  return ordered;
}

function outputPathForModule(outDir: string, key: string): string {
  return resolve(outDir, key.replace(/\.ns$/i, ".js"));
}

function relativeModuleSpecifier(fromOutput: string, targetOutput: string): string {
  const value = relative(dirname(fromOutput), targetOutput).split(sep).join("/");
  return value.startsWith(".") ? value : `./${value}`;
}

function runBuild(args: readonly string[], cwd: string): number {
  if (args.includes("-h") || args.includes("--help")) {
    printHelp();
    return 0;
  }

  let options: BuildOptions;
  try {
    options = parseBuildArgs(args, cwd);
  } catch (error) {
    const diagnostic = errorDiagnostic(error);
    if (isJsonFlag(args)) {
      printBuildReport({
        target: "(default)",
        backend: "js",
        entry: "",
        output: null,
        diagnostics: [diagnostic],
      });
    } else {
      printDiagnostic(diagnostic);
    }
    return 1;
  }

  let report: BuildReport = {
    target: options.target ?? "(default)",
    backend: "js",
    entry: "",
    output: null,
    diagnostics: [],
  };

  try {
    const manifest = loadManifest(options.project);
    const root = dirname(manifest.file.path);
    const build = isRecord(manifest.data["build"]) ? manifest.data["build"] : {};
    const targets = isRecord(manifest.data["targets"])
      ? manifest.data["targets"]
      : {};
    const targetName =
      options.target ??
      process.env.ASTRA_TARGET ??
      (typeof build["defaultTarget"] === "string" ? build["defaultTarget"] : undefined);

    if (targetName === undefined || targetName.length === 0) {
      throw new CliError(
        "NS9008",
        "No build target was selected. Set --target, ASTRA_TARGET, or build.defaultTarget.",
        displayPath(manifest.file.path, cwd),
      );
    }

    const targetConfig = targets[targetName];
    if (!isRecord(targetConfig)) {
      throw new CliError(
        "NS8001",
        `Target '${targetName}' is not declared in the manifest.`,
        displayPath(manifest.file.path, cwd),
      );
    }

    const backend = targetConfig["backend"];
    if (backend !== "js") {
      throw new CliError(
        "NS8001",
        `Target '${targetName}' uses backend '${String(backend)}'; astra build currently supports only the 'js' backend.`,
        displayPath(manifest.file.path, cwd),
      );
    }

    const moduleFormat = targetConfig["module"] ?? "esm";
    if (moduleFormat !== "esm") {
      throw new CliError(
        "NS8001",
        `Target '${targetName}' requests module format '${String(moduleFormat)}'; only 'esm' is supported.`,
        displayPath(manifest.file.path, cwd),
      );
    }

    const nova = isRecord(manifest.data["nova"]) ? manifest.data["nova"] : {};
    const entryValue = nova["entry"];
    if (typeof entryValue !== "string" || entryValue.length === 0) {
      throw new CliError(
        "NV0002",
        "The manifest does not declare nova.entry.",
        displayPath(manifest.file.path, cwd),
      );
    }

    const entryPath = resolve(root, entryValue);
    const relativeEntry = relative(root, entryPath);
    if (
      relativeEntry.length === 0 ||
      isAbsolute(relativeEntry) ||
      relativeEntry === ".." ||
      relativeEntry.startsWith(`..${sep}`)
    ) {
      throw new CliError(
        "NS9008",
        "The configured entry point must be inside the project.",
        displayPath(entryPath, cwd),
      );
    }
    if (extname(entryPath).toLowerCase() !== SOURCE_EXTENSION) {
      throw new CliError(
        "NS9008",
        "The configured entry point must be a .ns file.",
        displayPath(entryPath, cwd),
      );
    }

    const entryKey = relativeEntry.split(sep).join("/");
    const sourceFile: SourceFile = {
      path: entryPath,
      displayPath: displayPath(entryPath, cwd),
      kind: "source",
    };
    const projectSources = discoverProjectModules(root, entryPath, entryKey, cwd);
    const source =
      projectSources.find((module) => module.key === entryKey)?.text ??
      readSource(sourceFile);

    const modeValue = targetConfig["mode"] ?? build["defaultMode"];
    if (modeValue !== undefined && !isMode(modeValue)) {
      throw new CliError(
        "NS9008",
        `Unknown mode '${String(modeValue)}' for target '${targetName}'.`,
        displayPath(manifest.file.path, cwd),
      );
    }

    const math = isRecord(manifest.data["math"]) ? manifest.data["math"] : {};
    const policyValue = math["integers"];
    if (policyValue !== undefined && !isIntegerPolicy(policyValue)) {
      throw new CliError(
        "NS9008",
        `Unknown integer policy '${String(policyValue)}'.`,
        displayPath(manifest.file.path, cwd),
      );
    }

    const client = new CompilerClient({
      cwd: root,
      ...(options.compilerPath === undefined
        ? {}
        : { binaryPath: options.compilerPath }),
    });
    const result = client.checkProject({
      sources: projectSources.map((module) => ({
        key: module.key,
        path: module.key,
        text: module.text,
      })),
      target: "js",
      ...(modeValue === undefined ? {} : { mode: modeValue }),
      ...(policyValue === undefined ? {} : { integerPolicy: policyValue }),
    });

    const moduleSources = new Map(
      projectSources.map((module) => [module.key, module] as const),
    );
    const diagnostics = result.modules.flatMap((module) => {
      const moduleSource = moduleSources.get(module.key);
      const file: SourceFile = {
        path: moduleSource?.absolutePath ?? entryPath,
        displayPath:
          moduleSource === undefined
            ? displayPath(entryPath, cwd)
            : displayPath(moduleSource.absolutePath, cwd),
        kind: "source",
      };
      return module.diagnostics.map((diagnostic) =>
        reportDiagnostic(file, moduleSource?.text ?? source, diagnostic),
      );
    });
    const errors = diagnostics.filter((diagnostic) => diagnostic.severity === "error");
    if (errors.length > 0) {
      report = { ...report, target: targetName, entry: entryKey, diagnostics };
      if (options.json) {
        printBuildReport(report);
      } else {
        for (const diagnostic of diagnostics) {
          printDiagnostic(diagnostic);
        }
      }
      return 1;
    }

    const missingModule = result.modules.find(
      (module) => module.ok && module.ir === undefined,
    );
    if (missingModule !== undefined) {
      throw new CliError(
        "NS8001",
        `The compiler returned no typed module for '${missingModule.key}'.`,
        displayPath(entryPath, cwd),
      );
    }

    const configuredOutDir = targetConfig["outDir"] ?? build["outDir"];
    const outDir =
      typeof configuredOutDir === "string"
        ? resolve(root, configuredOutDir)
        : resolve(root, "dist", targetName);
    const outputs = new Map(
      projectSources.map((module) => [
        module.key,
        outputPathForModule(outDir, module.key),
      ]),
    );

    const generatedModules = result.modules
      .filter(
        (module): module is typeof module & { readonly ir: NonNullable<typeof module.ir> } =>
          module.ir !== undefined,
      )
      .map((module) => {
        const outputPath = outputs.get(module.key);
        const moduleSource = moduleSources.get(module.key);
        if (outputPath === undefined || moduleSource === undefined) {
          throw new CliError(
            "NS8001",
            `No output path was resolved for '${module.key}'.`,
            displayPath(entryPath, cwd),
          );
        }
        const moduleSpecifiers = Object.fromEntries(
          projectSources.map((candidate) => [
            candidate.key,
            relativeModuleSpecifier(
              outputPath,
              outputs.get(candidate.key) ??
                outputPathForModule(outDir, candidate.key),
            ),
          ]),
        );
        const generated = generateJavaScript({
          moduleName: module.key,
          ir: module.ir,
          sourceText: moduleSource.text,
          moduleSpecifiers,
        });
        return {
          module,
          moduleSource,
          outputPath,
          generated,
        };
      });

    const generatedDiagnostics = generatedModules.flatMap((entry) =>
      entry.generated.diagnostics.map((diagnostic) =>
        codegenDiagnostic(
          {
            path: entry.moduleSource.absolutePath,
            displayPath: displayPath(entry.moduleSource.absolutePath, cwd),
            kind: "source",
          },
          entry.moduleSource.text,
          diagnostic,
        ),
      ),
    );
    const allDiagnostics = [...diagnostics, ...generatedDiagnostics];
    if (generatedDiagnostics.length > 0) {
      report = {
        ...report,
        target: targetName,
        entry: entryKey,
        diagnostics: allDiagnostics,
      };
      if (options.json) {
        printBuildReport(report);
      } else {
        for (const diagnostic of allDiagnostics) {
          printDiagnostic(diagnostic);
        }
      }
      return 1;
    }

    const outputPath = outputs.get(entryKey);
    if (outputPath === undefined) {
      throw new CliError(
        "NS8001",
        "The entry module was not included in the project output.",
        displayPath(entryPath, cwd),
      );
    }

    try {
      for (const entry of generatedModules) {
        mkdirSync(dirname(entry.outputPath), { recursive: true });
        const mapName = `${basename(entry.outputPath)}.map`;
        const generatedCode =
          entry.generated.sourceMap === undefined
            ? entry.generated.code
            : `${entry.generated.code}//# sourceMappingURL=${mapName}\n`;
        writeFileSync(entry.outputPath, generatedCode, "utf8");
        if (entry.generated.sourceMap !== undefined) {
          const map = JSON.parse(entry.generated.sourceMap) as Record<string, unknown>;
          map["file"] = basename(entry.outputPath);
          map["sources"] = [
            relative(
              dirname(entry.outputPath),
              entry.moduleSource.absolutePath,
            )
              .split(sep)
              .join("/"),
          ];
          writeFileSync(
            `${entry.outputPath}.map`,
            `${JSON.stringify(map)}\n`,
            "utf8",
          );
        }
      }
      const packagePath = join(outDir, "package.json");
      if (!existsSync(packagePath)) {
        writeFileSync(packagePath, '{ "type": "module" }\n', "utf8");
      }
    } catch (error) {
      throw new CliError(
        "NS9008",
        `Unable to write the generated module: ${errorMessage(error)}`,
        displayPath(outputPath, cwd),
      );
    }

    report = {
      target: targetName,
      backend: "js",
      entry: entryKey,
      output: displayPath(outputPath, cwd),
      diagnostics: allDiagnostics,
    };
    if (options.json) {
      printBuildReport(report);
    } else {
      for (const diagnostic of allDiagnostics) {
        printDiagnostic(diagnostic);
      }
      process.stdout.write(`built ${targetName} -> ${displayPath(outputPath, cwd)}\n`);
    }
    return 0;
  } catch (error) {
    const diagnostic = errorDiagnostic(error);
    report = { ...report, diagnostics: [diagnostic] };
    if (options.json) {
      printBuildReport(report);
    } else {
      printDiagnostic(diagnostic);
    }
    return 1;
  }
}

function runNotImplemented(command: string): number {
  process.stderr.write(`astra ${command}: not implemented in the scaffold.\n`);
  return 2;
}

export async function runCli(args: readonly string[], cwd = process.cwd()): Promise<number> {
  const [command, ...rest] = args;

  if (command === undefined || command === "-h" || command === "--help" || command === "help") {
    printHelp();
    return 0;
  }

  if (command === "-v" || command === "--version" || command === "version") {
    printVersion();
    return 0;
  }

  switch (command) {
    case "check":
      return runCheck(rest, cwd);
    case "fmt":
      return runFmt(rest, cwd);
    case "build":
      return runBuild(rest, cwd);
    case "ast":
    case "explain":
      return runNotImplemented(command);
    default:
      process.stderr.write(`astra: unknown command '${command}'.\n\n`);
      printHelp();
      return 1;
  }
}

const entry = process.argv[1];
if (entry !== undefined && import.meta.url === pathToFileURL(entry).href) {
  process.exitCode = await runCli(process.argv.slice(2));
}
