export interface CodegenRequest {
  readonly moduleName: string;
  readonly ir: unknown;
}

export interface CodegenResult {
  readonly code: string;
  readonly sourceMap?: string;
}

export class CodegenNotImplementedError extends Error {
  readonly target = "ts";

  constructor() {
    super("The TypeScript backend is not implemented in the scaffold.");
    this.name = "CodegenNotImplementedError";
  }
}

export function generateTypeScript(_request: CodegenRequest): CodegenResult {
  throw new CodegenNotImplementedError();
}
