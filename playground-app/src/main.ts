import {
  projectSettings,
  validateManifestFiles,
  type ManifestIssue,
} from "./manifest.js";
import { ProjectStore, loadProject, saveProject, type ProjectFile } from "./project.js";
import { generateLockfile } from "./lockfile.js";
import { PlaygroundCompiler } from "./compiler-client.js";
import {
  COMPILER_API_VERSION,
  PLAYGROUND_PROTOCOL_VERSION,
  type AstResult,
  type ExplainResult,
  type FormatResult,
  type ProjectCheckResult,
  type ProjectSource,
  type StructuredDiagnostic,
} from "@neo-astra/playground/protocol";

function element<K extends keyof HTMLElementTagNameMap>(
  tag: K,
  className?: string,
  text?: string,
): HTMLElementTagNameMap[K] {
  const node = document.createElement(tag);
  if (className !== undefined) node.className = className;
  if (text !== undefined) node.textContent = text;
  return node;
}

const root = document.getElementById("app");
if (root === null) throw new Error("Missing #app element");

const header = element("header", "header");
header.append(element("h1", undefined, "Neo Astra Playground"));
header.append(
  element("span", "badge", `protocol ${String(PLAYGROUND_PROTOCOL_VERSION)}`),
);
root.append(header);

const layout = element("div", "layout");
const sidebar = element("aside", "panel sidebar");
const editorPane = element("section", "panel editor-pane");
const infoPane = element("aside", "panel info-pane");
layout.append(sidebar, editorPane, infoPane);
root.append(layout);

const status = element("div", "status");
root.append(status);

function setStatus(message: string, isError = false): void {
  status.textContent = message;
  status.classList.toggle("error", isError);
}

const project = new ProjectStore(
  () => render(),
  loadProject() ?? [],
);
const compiler = new PlaygroundCompiler();

const fileHeading = element("h2", undefined, "Files");
const fileList = element("ul", "file-list");
sidebar.append(fileHeading, fileList);

const addRow = element("div", "button-row");
const addSourceButton = element("button", undefined, "Add .ns file");
const addCoreJsonButton = element("button", undefined, "core.json");
const addCoreTomlButton = element("button", undefined, "core.toml");
const addBuildJsonButton = element("button", undefined, "build.json");
const addBuildTomlButton = element("button", undefined, "build.toml");
addRow.append(
  addSourceButton,
  addCoreJsonButton,
  addCoreTomlButton,
  addBuildJsonButton,
  addBuildTomlButton,
);
sidebar.append(addRow);

addSourceButton.addEventListener("click", () => {
  const path = window.prompt("New source path", "src/new_file.ns");
  if (path === null) return;
  try {
    project.add(path, "module app.main\n\npub fn main() {\n}\n");
  } catch (error) {
    setStatus(error instanceof Error ? error.message : String(error), true);
  }
});

function addManifest(name: string): void {
  try {
    project.add(name, name.endsWith(".json") ? "{\n}\n" : "");
  } catch (error) {
    setStatus(error instanceof Error ? error.message : String(error), true);
  }
}
addCoreJsonButton.addEventListener("click", () => addManifest("core.json"));
addCoreTomlButton.addEventListener("click", () => addManifest("core.toml"));
addBuildJsonButton.addEventListener("click", () => addManifest("build.json"));
addBuildTomlButton.addEventListener("click", () => addManifest("build.toml"));

const editorHeader = element("div", "editor-header");
const activePathLabel = element("span", "active-path");
const renameButton = element("button", undefined, "Rename");
const deleteButton = element("button", undefined, "Delete");
editorHeader.append(activePathLabel, renameButton, deleteButton);

const editor = element("textarea", "editor");
editor.spellcheck = false;
editorPane.append(editorHeader, editor);

renameButton.addEventListener("click", () => {
  const next = window.prompt("Rename file to", project.activePath);
  if (next === null) return;
  try {
    project.rename(project.activePath, next);
  } catch (error) {
    setStatus(error instanceof Error ? error.message : String(error), true);
  }
});

deleteButton.addEventListener("click", () => {
  if (!window.confirm(`Delete '${project.activePath}'?`)) return;
  try {
    project.remove(project.activePath);
  } catch (error) {
    setStatus(error instanceof Error ? error.message : String(error), true);
  }
});

editor.addEventListener("input", () => {
  try {
    project.write(project.activePath, editor.value);
    saveProject(project.getFiles());
  } catch (error) {
    setStatus(error instanceof Error ? error.message : String(error), true);
  }
});

const diagnosticsHeading = element("h2", undefined, "Manifest diagnostics");
const diagnosticList = element("ul", "diagnostics");
infoPane.append(diagnosticsHeading, diagnosticList);

const lockHeading = element("h2", undefined, "core.lock.json");
const lockButton = element("button", undefined, "Generate from files");
const lockOutput = element("pre", "lock-output", "No lockfile generated yet.");
infoPane.append(lockHeading, lockButton, lockOutput);

lockButton.addEventListener("click", () => {
  void generateLockfile(project.getFiles())
    .then((text) => {
      lockOutput.textContent = text;
      setStatus("Generated core.lock.json preview.");
    })
    .catch((error: unknown) => {
      setStatus(error instanceof Error ? error.message : String(error), true);
    });
});

const compilerHeading = element("h2", undefined, "Compiler");
const compilerRow = element("div", "button-row");
const checkButton = element("button", undefined, "Check project");
const formatButton = element("button", undefined, "Format");
const astButton = element("button", undefined, "AST");
const explainButton = element("button", undefined, "Explain");
compilerRow.append(checkButton, formatButton, astButton, explainButton);

const compilerDiagnosticsHeading = element("h2", undefined, "Compiler diagnostics");
const compilerDiagnosticList = element("ul", "diagnostics");
const compilerOutputHeading = element("h2", undefined, "Compiler output");
const compilerOutput = element("pre", "lock-output", "Run a compiler action.");
infoPane.append(
  compilerHeading,
  compilerRow,
  compilerDiagnosticsHeading,
  compilerDiagnosticList,
  compilerOutputHeading,
  compilerOutput,
);

interface CompilerParams {
  readonly apiVersion: typeof COMPILER_API_VERSION;
  readonly target: string;
  readonly edition: "0.1" | "0.2";
  readonly mode?: "core" | "app" | "web" | "system";
  readonly integerPolicy?: "wrapping" | "safe-checked" | "arbitrary";
}

function baseParams(): CompilerParams {
  const settings = projectSettings(validateManifestFiles(project.snapshot()));
  return {
    apiVersion: COMPILER_API_VERSION,
    target: settings.target,
    edition: settings.edition,
    ...(settings.mode === undefined ? {} : { mode: settings.mode }),
    ...(settings.integerPolicy === undefined ? {} : { integerPolicy: settings.integerPolicy }),
  };
}

function sourceUnits(): readonly ProjectSource[] {
  return project
    .getFiles()
    .filter((file) => file.path.endsWith(".ns"))
    .sort((a, b) => {
      const aEntry = a.path.includes("main") ? 1 : 0;
      const bEntry = b.path.includes("main") ? 1 : 0;
      return aEntry - bEntry || a.path.localeCompare(b.path);
    })
    .map((file) => ({ key: file.path, path: file.path, text: file.text }));
}

function showOutput(value: unknown): void {
  compilerOutput.textContent = JSON.stringify(value, null, 2);
}

async function runAction<Result>(label: string, action: () => Promise<Result>): Promise<Result | undefined> {
  setStatus(`${label}...`);
  try {
    const result = await action();
    setStatus(`${label} complete.`);
    return result;
  } catch (error) {
    setStatus(error instanceof Error ? error.message : String(error), true);
    return undefined;
  }
}

checkButton.addEventListener("click", () => {
  void runAction("Check project", async () => {
    const result = await compiler.request<ProjectCheckResult>("check_project", {
      ...baseParams(),
      sources: sourceUnits(),
    });
    renderCompilerDiagnostics(result.modules.flatMap((module) => module.diagnostics));
    showOutput(result);
    return result;
  });
});

function activeSource(): { readonly path: string; readonly text: string } | undefined {
  const path = project.activePath;
  if (!path.endsWith(".ns")) {
    setStatus("Select a .ns file first.", true);
    return undefined;
  }
  return { path, text: project.read(path) ?? "" };
}

formatButton.addEventListener("click", () => {
  const source = activeSource();
  if (source === undefined) return;
  void runAction("Format", async () => {
    const result = await compiler.request<FormatResult>("format", { ...baseParams(), source });
    renderCompilerDiagnostics(result.diagnostics);
    if (result.source !== null) {
      project.write(source.path, result.source);
    }
    showOutput(result);
    return result;
  });
});

astButton.addEventListener("click", () => {
  const source = activeSource();
  if (source === undefined) return;
  void runAction("AST", async () => {
    const result = await compiler.request<AstResult>("ast", { ...baseParams(), source });
    renderCompilerDiagnostics(result.diagnostics);
    showOutput(result);
    return result;
  });
});

explainButton.addEventListener("click", () => {
  const source = activeSource();
  if (source === undefined) return;
  void runAction("Explain", async () => {
    const result = await compiler.request<ExplainResult>("explain", { ...baseParams(), source });
    renderCompilerDiagnostics(result.diagnostics);
    showOutput(result.explanation ?? result);
    return result;
  });
});

interface DisplayIssue {
  readonly code: string;
  readonly message: string;
  readonly hint: string | null;
}

function renderIssueList(list: HTMLUListElement, issues: readonly DisplayIssue[], empty: string): void {
  list.replaceChildren();
  if (issues.length === 0) {
    list.append(element("li", "ok", empty));
    return;
  }
  for (const item of issues) {
    const node = element("li", "issue");
    node.append(element("code", undefined, item.code));
    node.append(document.createTextNode(` ${item.message}`));
    if (item.hint !== null && item.hint !== "") {
      node.append(document.createTextNode(` Hint: ${item.hint}`));
    }
    list.append(node);
  }
}

function renderCompilerDiagnostics(diagnostics: readonly StructuredDiagnostic[]): void {
  renderIssueList(
    compilerDiagnosticList,
    diagnostics.map((diagnostic) => ({
      code: diagnostic.code,
      message: diagnostic.span === null
        ? diagnostic.message
        : `${diagnostic.span.start}-${diagnostic.span.end}: ${diagnostic.message}`,
      hint: diagnostic.hint,
    })),
    "No compiler diagnostics.",
  );
}

function renderDiagnostics(issues: readonly ManifestIssue[]): void {
  renderIssueList(
    diagnosticList,
    issues.map((issue) => ({
      code: issue.code,
      message: `${issue.file}: ${issue.message}`,
      hint: null,
    })),
    "No manifest issues.",
  );
}

function renderFileList(): void {
  fileList.replaceChildren();
  for (const path of project.paths) {
    const item = element("li");
    const button = element("button", "file-button", path);
    button.classList.toggle("active", path === project.activePath);
    button.addEventListener("click", () => {
      try {
        project.select(path);
      } catch (error) {
        setStatus(error instanceof Error ? error.message : String(error), true);
      }
    });
    item.append(button);
    fileList.append(item);
  }
}

function render(): void {
  renderFileList();
  const active = project.activePath;
  activePathLabel.textContent = active === "" ? "(no files)" : active;
  const text = active === "" ? "" : (project.read(active) ?? "");
  if (editor.value !== text) editor.value = text;
  const validation = validateManifestFiles(project.snapshot());
  renderDiagnostics(validation.issues);
  const files: readonly ProjectFile[] = project.getFiles();
  saveProject(files);
}

render();
setStatus("S2 virtual project ready.");
