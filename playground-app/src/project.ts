/** In-memory virtual project used by the playground editors. */

export interface ProjectFile {
  readonly path: string;
  readonly text: string;
}

export const DEFAULT_FILES: readonly ProjectFile[] = [
  {
    path: "core.json",
    text: `{
  "package": {
    "name": "hello",
    "version": "1.0.0",
    "edition": "0.1"
  }
}
`,
  },
  {
    path: "build.json",
    text: `{
  "build": {
    "defaultTarget": "js",
    "defaultMode": "core"
  },
  "math": {
    "integers": "wrapping"
  },
  "strings": {
    "model": "utf8"
  }
}
`,
  },
  {
    path: "src/main.ns",
    text: `module app.main

pub fn main() {
    println("Hello from the playground")
}
`,
  },
];

export class ProjectStore {
  readonly #files = new Map<string, string>();
  #activePath: string;
  readonly #onChange: () => void;

  constructor(onChange: () => void, seed: readonly ProjectFile[] = DEFAULT_FILES) {
    this.#onChange = onChange;
    for (const file of seed) this.#files.set(file.path, file.text);
    this.#activePath = seed[0]?.path ?? "core.json";
  }

  get activePath(): string {
    return this.#activePath;
  }

  get paths(): readonly string[] {
    return [...this.#files.keys()].sort((a, b) => a.localeCompare(b));
  }

  getFiles(): readonly ProjectFile[] {
    return this.paths.map((path) => ({ path, text: this.#files.get(path) as string }));
  }

  snapshot(): ReadonlyMap<string, string> {
    return new Map(this.#files);
  }

  read(path: string): string | undefined {
    return this.#files.get(path);
  }

  select(path: string): void {
    if (!this.#files.has(path)) throw new Error(`Unknown file '${path}'.`);
    this.#activePath = path;
    this.#onChange();
  }

  write(path: string, text: string): void {
    if (!this.#files.has(path)) throw new Error(`Unknown file '${path}'.`);
    this.#files.set(path, text);
    this.#onChange();
  }

  add(path: string, text = ""): void {
    const normalized = path.trim().replaceAll("\\", "/");
    if (normalized === "") throw new Error("File name is required.");
    if (this.#files.has(normalized)) throw new Error(`'${normalized}' already exists.`);
    this.#files.set(normalized, text);
    this.#activePath = normalized;
    this.#onChange();
  }

  rename(oldPath: string, newPath: string): void {
    const normalized = newPath.trim().replaceAll("\\", "/");
    if (normalized === "") throw new Error("File name is required.");
    if (!this.#files.has(oldPath)) throw new Error(`Unknown file '${oldPath}'.`);
    if (normalized !== oldPath && this.#files.has(normalized)) {
      throw new Error(`'${normalized}' already exists.`);
    }
    const text = this.#files.get(oldPath) as string;
    this.#files.delete(oldPath);
    this.#files.set(normalized, text);
    this.#activePath = normalized;
    this.#onChange();
  }

  remove(path: string): void {
    if (!this.#files.has(path)) throw new Error(`Unknown file '${path}'.`);
    this.#files.delete(path);
    if (this.#activePath === path) {
      this.#activePath = this.paths[0] ?? "";
    }
    this.#onChange();
  }

  load(files: readonly ProjectFile[]): void {
    this.#files.clear();
    for (const file of files) this.#files.set(file.path, file.text);
    this.#activePath = this.paths[0] ?? "";
    this.#onChange();
  }
}

const STORAGE_KEY = "neo-astra-playground-project-v1";

export function saveProject(files: readonly ProjectFile[]): void {
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(files));
  } catch {
    // Storage is best-effort; the in-memory project remains authoritative.
  }
}

export function loadProject(): readonly ProjectFile[] | undefined {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (raw === null) return undefined;
    const value: unknown = JSON.parse(raw);
    if (!Array.isArray(value)) return undefined;
    const result: ProjectFile[] = [];
    for (const entry of value) {
      if (
        typeof entry === "object" &&
        entry !== null &&
        typeof (entry as { path?: unknown }).path === "string" &&
        typeof (entry as { text?: unknown }).text === "string"
      ) {
        const record = entry as { path: string; text: string };
        result.push({ path: record.path, text: record.text });
      }
    }
    return result.length > 0 ? result : undefined;
  } catch {
    return undefined;
  }
}
