//! Synchronous command execution used by source adapters.

import { spawnSync } from "node:child_process";

export interface CommandResult {
  readonly status: number | null;
  readonly stdout: string;
  readonly stderr: string;
}

export interface CommandRunner {
  run(
    command: string,
    args: readonly string[],
    options?: { readonly cwd?: string; readonly stdio?: "pipe" | "inherit" },
  ): CommandResult;
}

export const defaultCommandRunner: CommandRunner = {
  run(command, args, options = {}) {
    const stdio = options.stdio ?? "pipe";
    const result = spawnSync(command, [...args], {
      cwd: options.cwd,
      encoding: stdio === "pipe" ? "utf8" : undefined,
      stdio: stdio === "inherit" ? "inherit" : ["pipe", "pipe", "pipe"],
      maxBuffer: 32 * 1024 * 1024,
      windowsHide: true,
    });
    return {
      status: result.status,
      stdout: typeof result.stdout === "string" ? result.stdout : (result.stdout?.toString("utf8") ?? ""),
      stderr: typeof result.stderr === "string" ? result.stderr : (result.stderr?.toString("utf8") ?? result.error?.message ?? ""),
    };
  },
};

export function commandName(name: string): string {
  return process.platform === "win32" && name === "npm" ? "npm.cmd" : name;
}
