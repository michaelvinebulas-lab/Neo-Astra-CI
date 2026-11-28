import {
  CompilerBridge,
  CompilerBridgeError,
  CompilerResponseError,
} from "@neo-astra/playground";
import type { WorkerRequest, WorkerResponse } from "@neo-astra/playground/protocol";

interface WorkerScope {
  readonly location: { readonly href: string };
  postMessage(message: WorkerResponse): void;
  addEventListener(
    type: "message",
    listener: (event: MessageEvent<WorkerRequest>) => void,
  ): void;
}

const scope = self as unknown as WorkerScope;
let bridge: CompilerBridge | undefined;

async function ensureBridge(): Promise<CompilerBridge> {
  if (bridge === undefined) {
    const url = new URL("./astra_wasm.wasm", scope.location.href);
    const response = await fetch(url);
    if (!response.ok) {
      throw new Error(`Failed to load compiler Wasm (${String(response.status)}).`);
    }
    bridge = await CompilerBridge.instantiate(await response.arrayBuffer());
  }
  return bridge;
}

function errorCode(error: unknown): string {
  if (error instanceof CompilerBridgeError) return error.code;
  if (error instanceof CompilerResponseError) return String(error.code);
  return "PGW0004";
}

function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

scope.addEventListener("message", (event) => {
  const request = event.data;
  void (async () => {
    try {
      let result: unknown = null;
      if (request.method === "shutdown") {
        bridge?.close();
        bridge = undefined;
      } else {
        const instance = await ensureBridge();
        result = await instance.request({
          jsonrpc: "2.0",
          id: request.id,
          method: request.method,
          params: request.params,
        });
      }
      scope.postMessage({ id: request.id, ok: true, result });
    } catch (error) {
      scope.postMessage({
        id: request.id,
        ok: false,
        error: { code: errorCode(error), message: errorMessage(error) },
      });
    }
  })();
});
