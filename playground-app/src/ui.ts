/** DOM shell for the playground. No compiler or project logic lives here. */

export function element<K extends keyof HTMLElementTagNameMap>(
  tag: K,
  className?: string,
  text?: string,
): HTMLElementTagNameMap[K] {
  const node = document.createElement(tag);
  if (className !== undefined) node.className = className;
  if (text !== undefined) node.textContent = text;
  return node;
}

export function button(label: string, onClick: () => void, title?: string): HTMLButtonElement {
  const node = element("button", undefined, label);
  if (title !== undefined) node.title = title;
  node.addEventListener("click", onClick);
  return node;
}

export interface SelectOption {
  readonly value: string;
  readonly label: string;
}

export function select(
  options: readonly SelectOption[],
  value: string,
  onChange: (next: string) => void,
  title?: string,
): HTMLSelectElement {
  const node = element("select");
  if (title !== undefined) node.title = title;
  for (const option of options) {
    const item = element("option", undefined, option.label);
    item.value = option.value;
    node.append(item);
  }
  node.value = value;
  node.addEventListener("change", () => onChange(node.value));
  return node;
}

export interface Shell {
  readonly badge: HTMLSpanElement;
  readonly settings: HTMLDivElement;
  readonly fileList: HTMLUListElement;
  readonly fileActions: HTMLDivElement;
  readonly projectActions: HTMLDivElement;
  readonly activePath: HTMLSpanElement;
  readonly editorMeta: HTMLSpanElement;
  readonly primaryActions: HTMLDivElement;
  readonly editor: HTMLTextAreaElement;
  readonly gutter: HTMLDivElement;
  readonly highlight: HTMLPreElement;
  readonly advancedActions: HTMLDivElement;
  readonly tabs: HTMLDivElement;
  readonly panels: HTMLDivElement;
  readonly guide: HTMLDivElement;
  readonly diagnostics: HTMLUListElement;
  readonly compilerDiagnostics: HTMLUListElement;
  readonly output: HTMLPreElement;
  readonly expected: HTMLTextAreaElement;
  readonly lockfile: HTMLPreElement;
  readonly status: HTMLDivElement;
}

export function buildShell(root: HTMLElement): Shell {
  const header = element("header", "header");
  header.append(element("h1", undefined, "Neo Astra Playground"));
  const badge = element("span", "badge");
  header.append(badge);
  root.append(header);

  const settings = element("div", "settings");
  root.append(settings);

  const layout = element("div", "layout");

  const sidebar = element("aside", "panel sidebar");
  sidebar.append(element("h2", undefined, "1. Project files"));
  const fileList = element("ul", "file-list");
  const fileActions = element("div", "button-row");
  sidebar.append(fileList, fileActions);
  sidebar.append(element("h2", undefined, "Project actions"));
  const projectActions = element("div", "button-row");
  sidebar.append(projectActions);

  const editorPane = element("section", "panel editor-pane");
  const editorHeader = element("div", "editor-header");
  const activePath = element("span", "active-path");
  const primaryActions = element("div", "button-row primary");
  const editorMeta = element("span", "editor-meta");
  editorHeader.append(activePath, primaryActions, editorMeta);

  const editorShell = element("div", "editor-shell");
  const gutter = element("div", "gutter");
  const highlight = element("pre", "highlight");
  const editor = element("textarea", "editor");
  editor.spellcheck = false;
  editor.wrap = "off";
  editorShell.append(gutter, highlight, editor);

  const advanced = element("details", "advanced");
  advanced.append(element("summary", undefined, "More tools"));
  const advancedActions = element("div", "button-row");
  advanced.append(advancedActions);
  editorPane.append(editorHeader, editorShell, advanced);

  const infoPane = element("aside", "panel info-pane");
  const tabs = element("div", "tabs");
  const panels = element("div", "panels");
  const guide = element("div", "panel-body guide");
  const diagnostics = element("ul", "diagnostics");
  const compilerDiagnostics = element("ul", "diagnostics");
  const output = element("pre", "panel-output", "Run an action to see output.");
  const expected = element("textarea", "expected", "");
  expected.placeholder = "Expected stdout (optional). Run reports pass/fail when set.";
  const lockfile = element("pre", "panel-output", "No lockfile generated yet.");
  infoPane.append(tabs, panels);

  layout.append(sidebar, editorPane, infoPane);
  root.append(layout);

  const status = element("div", "status");
  root.append(status);

  return {
    badge,
    settings,
    fileList,
    fileActions,
    projectActions,
    activePath,
    editorMeta,
    primaryActions,
    editor,
    gutter,
    highlight,
    advancedActions,
    tabs,
    panels,
    guide,
    diagnostics,
    compilerDiagnostics,
    output,
    expected,
    lockfile,
    status,
  };
}

export interface DisplayIssue {
  readonly code: string;
  readonly message: string;
  readonly hint: string | null;
}

export function renderIssues(
  list: HTMLUListElement,
  issues: readonly DisplayIssue[],
  empty: string,
): void {
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
