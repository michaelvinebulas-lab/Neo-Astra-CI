/** Deterministic browser-side lockfile generation for the virtual project. */

import type { ProjectFile } from "./project.js";

export interface LockedPackage {
  readonly id: string;
  readonly source: "path";
  readonly path: string;
  readonly integrity: string;
}

export interface Lockfile {
  readonly version: 1;
  readonly generatedBy: "neo-astra-playground";
  readonly packages: readonly LockedPackage[];
}

async function sha256(text: string): Promise<string> {
  const bytes = new TextEncoder().encode(text);
  const digest = await crypto.subtle.digest("SHA-256", bytes);
  return [...new Uint8Array(digest)]
    .map((byte) => byte.toString(16).padStart(2, "0"))
    .join("");
}

export async function generateLockfile(files: readonly ProjectFile[]): Promise<string> {
  const packages: LockedPackage[] = [];
  for (const file of files.filter((entry) => entry.path.endsWith(".ns")).sort((a, b) => a.path.localeCompare(b.path))) {
    packages.push({
      id: `path:${file.path}`,
      source: "path",
      path: file.path,
      integrity: `sha256-${await sha256(file.text)}`,
    });
  }
  const lockfile: Lockfile = {
    version: 1,
    generatedBy: "neo-astra-playground",
    packages,
  };
  return `${JSON.stringify(lockfile, null, 2)}\n`;
}
