export type LspTransport = "stdio";

export interface LspServerDescriptor {
  readonly name: string;
  readonly version: string;
  readonly transport: LspTransport;
  readonly capabilities: readonly string[];
}

export const LSP_CAPABILITIES = [
  "textDocument/diagnostic",
  "textDocument/formatting",
  "textDocument/hover",
  "textDocument/definition",
  "textDocument/references",
  "textDocument/completion",
] as const;

export function createLspStub(version = "0.1.0"): LspServerDescriptor {
  return {
    name: "neo-astra-lsp",
    version,
    transport: "stdio",
    capabilities: LSP_CAPABILITIES,
  };
}

export class LspNotImplementedError extends Error {
  constructor() {
    super("The Neo Astra language server is not implemented in the scaffold.");
    this.name = "LspNotImplementedError";
  }
}

export function startLsp(): Promise<never> {
  return Promise.reject(new LspNotImplementedError());
}
