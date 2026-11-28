import {
  projectSettings,
  validateManifestFiles,
  type ManifestIssue,
} from "./manifest.js";
import {
  ProjectStore,
  loadProject,
  saveProject,
  DEFAULT_FILES,
  type ProjectFile,
} from "./project.js";
import { generateLockfile } from "./lockfile.js";
import { PlaygroundCompiler } from "./compiler-client.js";
import {
  COMPILER_API_VERSION,
  PLAYGROUND_PROTOCOL_VERSION,
  type AstResult,
  type CanonicalResult,
  type ExplainResult,
  type FormatResult,
  type MigrateResult,
  type ProjectCheckResult,
  type ProjectSource,
  type StructuredDiagnostic,
} from "@neo-astra/playground/protocol";
import {
  DEFAULT_OVERRIDES,
  resolveSettings,
  withOptionalFields,
  type EditionOverride,
  type IntegerPolicyOverride,
  type ModeOverride,
  type UiOverrides,
} from "./settings.js";
import {
  buildShell,
  button,
  element,
  renderIssues,
  select,
  type DisplayIssue,
  type SelectOption,
} from "./ui.js";

const host = document.getElementById("app");
if (host === null) throw new Error("Missing #app element");

const shell = buildShell(host);
shell.badge.textContent = `protocol ${String(PLAYGROUND_PROTOCOL_VERSION)}`;

const project = new ProjectStore(() => render(), loadProject() ?? DEFAULT_FILES);
const compiler = new PlaygroundCompiler();
let overrides: UiOverrides = DEFAULT_OVERRIDES;
let compilerIssues: readonly DisplayIssue[] = [];

function setStatus(message: string, isError = false): void {
  shell.status.textContent = message;
  shell.status.classList.toggle("error", isError);
}

function guard(action: () => void): void {
  try {
    action();
  } catch (error) {
    setStatus(error instanceof Error ? error.message : String(error), true);
  }
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function asDiagnostics(value: unknown): readonly StructuredDiagnostic[] {
  if (!isRecord(value)) return [];
  const direct = value["diagnostics"];
  if (Array.isArray(direct)) return direct as readonly StructuredDiagnostic[];
  const modules = value["modules"];
  if (!Array.isArray(modules)) return [];
  return modules.flatMap((module) =>
    isRecord(module) && Array.isArray(module["diagnostics"])
      ? (module["diagnostics"] as readonly StructuredDiagnostic[])
      : [],
  );
}

function setCompilerDiagnostics(diagnostics: readonly StructuredDiagnostic[]): void {
  compilerIssues = diagnostics.map((diagnostic) => ({
    code: diagnostic.code,
    message:
      diagnostic.span === null
        ? diagnostic.message
        : `${diagnostic.span.start}-${diagnostic.span.end}: ${diagnostic.message}`,
    hint: diagnostic.hint,
  }));
  renderIssues(shell.compilerDiagnostics, compilerIssues, "No compiler diagnostics.");
}

// ── Settings controls ────────────────────────────────────────────────────────

function addSetting(label: string, control: HTMLElement): void {
  const wrapper = element("label", "setting");
  wrapper.append(element("span", undefined, label), control);
  shell.settings.append(wrapper);
}

const editionOptions: readonly SelectOption[] = [
  { value: "auto", label: "edition: auto" },
  { value: "0.1", label: "edition: 0.1" },
  { value: "0.2", label: "edition: 0.2" },
];
const targetOptions: readonly SelectOption[] = [
  { value: "", label: "target: auto" },
  { value: "js", label: "js" },
  { value: "ts", label: "ts" },
  { value: "native", label: "native" },
  { value: "wasm-web", label: "wasm-web" },
  { value: "wasm-wasi", label: "wasm-wasi" },
];
const modeOptions: readonly SelectOption[] = [
  { value: "auto", label: "mode: auto" },
  { value: "core", label: "core" },
  { value: "app", label: "app" },
  { value: "web", label: "web" },
  { value: "system", label: "system" },
];
const policyOptions: readonly SelectOption[] = [
  { value: "auto", label: "integers: auto" },
  { value: "wrapping", label: "wrapping" },
  { value: "safe-checked", label: "safe-checked" },
  { value: "arbitrary", label: "arbitrary" },
];

addSetting(
  "Language",
  select(editionOptions, overrides.edition, (value) => {
    overrides = { ...overrides, edition: value as EditionOverride };
    render();
  }),
);
addSetting(
  "Target",
  select(targetOptions, overrides.target, (value) => {
    overrides = { ...overrides, target: value };
    render();
  }),
);
addSetting(
  "Mode",
  select(modeOptions, overrides.mode, (value) => {
    overrides = { ...overrides, mode: value as ModeOverride };
    render();
  }),
);
addSetting(
  "Integers",
  select(policyOptions, overrides.integerPolicy, (value) => {
    overrides = { ...overrides, integerPolicy: value as IntegerPolicyOverride };
    render();
  }),
);

// ── File and project actions ─────────────────────────────────────────────────

function addFile(name: string, template: string): void {
  guard(() => {
    const path = name === "source" ? window.prompt("New source path", "src/new_file.ns") : name;
    if (path === null) return;
    project.add(path, template);
  });
}

shell.fileActions.append(
  button("+ source", () => addFile("source", "module app.main\n\npub fn main() {\n}\n")),
  button("+ core.json", () => addFile("core.json", "{\n}\n")),
  button("+ core.toml", () => addFile("core.toml", "")),
  button("+ build.json", () => addFile("build.json", "{\n}\n")),
  button("+ build.toml", () => addFile("build.toml", "")),
);

shell.dataActions.append(
  button("Generate lockfile", () => {
    void generateLockfile(project.getFiles())
      .then((text) => {
        shell.lockfile.textContent = text;
        showTab("lockfile");
        setStatus("Generated core.lock.json preview.");
      })
      .catch((error: unknown) => setStatus(String(error), true));
  }),
  button("Load example", () => guard(() => project.load(DEFAULT_FILES))),
  button("Download", () => {
    const body = JSON.stringify(project.getFiles(), null, 2);
    const url = URL.createObjectURL(new Blob([body], { type: "application/json" }));
    const link = element("a");
    link.href = url;
    link.download = "neo-astra-project.json";
    link.click();
    URL.revokeObjectURL(url);
  }),
);

// ── Panel tabs ───────────────────────────────────────────────────────────────

type TabName = "diagnostics" | "compiler" | "output" | "lockfile";
let activeTab: TabName = "diagnostics";

const tabDefinitions: readonly { readonly key: TabName; readonly label: string }[] = [
  { key: "diagnostics", label: "Manifest" },
  { key: "compiler", label: "Compiler" },
  { key: "output", label: "Output" },
  { key: "lockfile", label: "Lockfile" },
];

const panels: Record<TabName, HTMLElement> = {
  diagnostics: element("div", "panel-body"),
  compiler: element("div", "panel-body"),
  output: element("div", "panel-body"),
  lockfile: element("div", "panel-body"),
};
panels.diagnostics.append(shell.diagnostics);
panels.compiler.append(shell.compilerDiagnostics);
panels.output.append(shell.output);
panels.lockfile.append(shell.lockfile);
shell.panels.append(panels.diagnostics, panels.compiler, panels.output, panels.lockfile);

function showTab(name: TabName): void {
  activeTab = name;
  for (const definition of tabDefinitions) {
    const isActive = definition.key === activeTab;
    panels[definition.key].classList.toggle("hidden", !isActive);
    const tab = shell.tabs.querySelector<HTMLButtonElement>(`[data-tab="${definition.key}"]`);
    tab?.classList.toggle("active", isActive);
  }
}

for (const definition of tabDefinitions) {
  const tab = button(definition.label, () => showTab(definition.key));
  tab.dataset["tab"] = definition.key;
  shell.tabs.append(tab);
}

// ── Compiler actions ─────────────────────────────────────────────────────────

function effective() {
  return resolveSettings(projectSettings(validateManifestFiles(project.snapshot())), overrides);
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

function activeSource(): { readonly path: string; readonly text: string } | undefined {
  const path = project.activePath;
  if (!path.endsWith(".ns")) {
    setStatus("Select a .ns file first.", true);
    return undefined;
  }
  return { path, text: project.read(path) ?? "" };
}

function sourceParams(source: { readonly path: string; readonly text: string }) {
  return { ...withOptionalFields(effective()), source };
}

async function perform<Result>(
  label: string,
  method: "check_project" | "format" | "ast" | "explain" | "canonical" | "migrate",
  params: unknown,
  onResult: (result: Result) => void,
): Promise<void> {
  setStatus(`${label}...`);
  try {
    const result = await compiler.request<Result>(method, params);
    setCompilerDiagnostics(asDiagnostics(result));
    onResult(result);
    setStatus(`${label} complete.`);
  } catch (error) {
    setStatus(error instanceof Error ? error.message : String(error), true);
  }
}

function diffText(before: string, after: string): string {
  const oldLines = before.split("\n");
  const newLines = after.split("\n");
  let start = 0;
  while (start < oldLines.length && start < newLines.length && oldLines[start] === newLines[start]) {
    start++;
  }
  let oldEnd = oldLines.length - 1;
  let newEnd = newLines.length - 1;
  while (oldEnd >= start && newEnd >= start && oldLines[oldEnd] === newLines[newEnd]) {
    oldEnd--;
    newEnd--;
  }
  const lines = [
    ...oldLines.slice(0, start).map((line) => ` ${line}`),
    ...oldLines.slice(start, oldEnd + 1).map((line) => `-${line}`),
    ...newLines.slice(start, newEnd + 1).map((line) => `+${line}`),
    ...newLines.slice(newEnd + 1).map((line) => ` ${line}`),
  ];
  return lines.join("\n");
}

function promoteManifestEdition(): void {
  for (const path of ["core.json", "core.toml"]) {
    const text = project.read(path);
    if (text === undefined) continue;
    const next = path.endsWith(".json")
      ? text.replace('"edition": "0.1"', '"edition": "0.2"')
      : text.replace('edition = "0.1"', 'edition = "0.2"');
    if (next !== text) project.write(path, next);
  }
}

shell.actions.append(
  button("Check project", () => {
    void perform<ProjectCheckResult>(
      "Check project",
      "check_project",
      { ...withOptionalFields(effective()), sources: sourceUnits() },
      (result) => {
        shell.output.textContent = JSON.stringify(result, null, 2);
        showTab("output");
      },
    );
  }),
  button("Format", () => {
    const source = activeSource();
    if (source === undefined) return;
    void perform<FormatResult>("Format", "format", sourceParams(source), (result) => {
      if (result.source !== null) project.write(source.path, result.source);
      shell.output.textContent = JSON.stringify(result, null, 2);
      showTab("output");
    });
  }),
  button("AST", () => {
    const source = activeSource();
    if (source === undefined) return;
    void perform<AstResult>("AST", "ast", sourceParams(source), (result) => {
      shell.output.textContent = JSON.stringify(result, null, 2);
      showTab("output");
    });
  }),
  button("Explain", () => {
    const source = activeSource();
    if (source === undefined) return;
    void perform<ExplainResult>("Explain", "explain", sourceParams(source), (result) => {
      shell.output.textContent = result.explanation ?? JSON.stringify(result, null, 2);
      showTab("output");
    });
  }),
  button("Canonical lint", () => {
    const source = activeSource();
    if (source === undefined) return;
    void perform<CanonicalResult>("Canonical lint", "canonical", sourceParams(source), (result) => {
      shell.output.textContent = JSON.stringify(result, null, 2);
      showTab("compiler");
    });
  }),
  button("Migrate preview", () => {
    const source = activeSource();
    if (source === undefined) return;
    void perform<MigrateResult>(
      "Migrate preview",
      "migrate",
      { ...sourceParams(source), changeEdition: false },
      (result) => {
        shell.output.textContent = diffText(source.text, result.source);
        showTab("output");
      },
    );
  }),
  button("Apply migration", () => {
    const source = activeSource();
    if (source === undefined) return;
    void perform<MigrateResult>(
      "Apply migration",
      "migrate",
      { ...sourceParams(source), changeEdition: true },
      (result) => {
        project.write(source.path, result.source);
        overrides = { ...overrides, edition: "0.2" };
        promoteManifestEdition();
        shell.output.textContent = diffText(source.text, result.source);
        showTab("output");
      },
    );
  }),
);

// ── Editor and rendering ─────────────────────────────────────────────────────

shell.editor.addEventListener("input", () => {
  guard(() => {
    project.write(project.activePath, shell.editor.value);
    saveProject(project.getFiles());
    render();
  });
});

function renderFileList(): void {
  shell.fileList.replaceChildren();
  for (const path of project.paths) {
    const item = element("li");
    const kind = path.endsWith(".ns") ? "source" : path.includes("lock") ? "lock" : "manifest";
    const node = button(path, () => guard(() => project.select(path)));
    node.className = `file-button ${kind}`;
    node.classList.toggle("active", path === project.activePath);
    item.append(node);
    shell.fileList.append(item);
  }
}

function render(): void {
  renderFileList();
  const active = project.activePath;
  shell.activePath.textContent = active === "" ? "(no files)" : active;
  const text = active === "" ? "" : (project.read(active) ?? "");
  if (shell.editor.value !== text) shell.editor.value = text;
  const lines = text === "" ? 0 : text.split("\n").length;
  shell.editorMeta.textContent = `${String(lines)} lines - ${String(text.length)} chars`;

  const validation = validateManifestFiles(project.snapshot());
  renderIssues(
    shell.diagnostics,
    validation.issues.map((issue: ManifestIssue) => ({
      code: issue.code,
      message: `${issue.file}: ${issue.message}`,
      hint: null,
    })),
    "No manifest issues.",
  );
  renderIssues(shell.compilerDiagnostics, compilerIssues, "No compiler diagnostics.");
  saveProject(project.getFiles());
}

render();
showTab(activeTab);
setStatus(`S4 ready. API ${COMPILER_API_VERSION}.`);
