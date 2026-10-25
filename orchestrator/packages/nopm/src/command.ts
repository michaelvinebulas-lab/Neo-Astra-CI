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
    options?: { readonly cwd?: string },
  ): CommandResult;
}

export const defaultCommandRunner: CommandRunner = {
  run(command, args, options = {}) {
    const result = spawnSync(command, [...args], {
      cwd: options.cwd,
      encoding: "utf8",
      maxBuffer: 32 * 1024 * 1024,
      windowsHide: true,
    });
    return {
      status: result.status,
      stdout: result.stdout ?? "",
      stderr: result.stderr ?? result.error?.message ?? "",
    };
  },
};

export function commandName(name: string): string {
  return process.platform === "win32" && name === "npm" ? "npm.cmd" : name;
}
