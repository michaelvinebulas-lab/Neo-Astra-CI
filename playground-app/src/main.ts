import { validateManifestFiles, type ManifestIssue } from "./manifest.js";
import { ProjectStore, loadProject, saveProject, type ProjectFile } from "./project.js";
import { generateLockfile } from "./lockfile.js";
import { PLAYGROUND_PROTOCOL_VERSION } from "./protocol.js";

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

function renderDiagnostics(issues: readonly ManifestIssue[]): void {
  diagnosticList.replaceChildren();
  if (issues.length === 0) {
    diagnosticList.append(element("li", "ok", "No manifest issues."));
    return;
  }
  for (const issue of issues) {
    const item = element("li", "issue");
    item.append(element("code", undefined, issue.code));
    item.append(document.createTextNode(` ${issue.file}: ${issue.message}`));
    diagnosticList.append(item);
  }
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
