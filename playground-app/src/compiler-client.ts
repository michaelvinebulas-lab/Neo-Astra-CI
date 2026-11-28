import type {
  CompilerMethod,
  WorkerRequest,
  WorkerResponse,
} from "@neo-astra/playground/protocol";

interface PendingRequest {
  readonly resolve: (value: unknown) => void;
  readonly reject: (error: Error) => void;
  readonly timer: ReturnType<typeof setTimeout>;
}

/** Main-thread facade over the compiler Worker with hard timeouts. */
export class PlaygroundCompiler {
  readonly #timeoutMs: number;
  #worker: Worker;
  #nextId = 1;
  readonly #pending = new Map<number, PendingRequest>();

  constructor(timeoutMs = 15000) {
    this.#timeoutMs = timeoutMs;
    this.#worker = this.#spawn();
  }

  request<Result = unknown>(method: CompilerMethod, params: unknown): Promise<Result> {
    const id = this.#nextId++;
    const message: WorkerRequest = { id, method, params };
    return new Promise<Result>((resolve, reject) => {
      const timer = setTimeout(() => {
        this.#pending.delete(id);
        this.#restart(`Compiler request ${String(id)} timed out.`);
        reject(new Error("PGW0002: compiler request timed out."));
      }, this.#timeoutMs);
      this.#pending.set(id, {
        resolve: resolve as (value: unknown) => void,
        reject,
        timer,
      });
      this.#worker.postMessage(message);
    });
  }

  close(): void {
    this.#restart("Compiler client closed.");
  }

  #spawn(): Worker {
    const worker = new Worker(new URL("./compiler-worker.js", import.meta.url), {
      type: "module",
    });
    worker.addEventListener("message", (event: MessageEvent<WorkerResponse>) => {
      this.#receive(event.data);
    });
    worker.addEventListener("error", () => {
      this.#restart("Compiler worker crashed.");
    });
    return worker;
  }

  #receive(response: WorkerResponse): void {
    const pending = this.#pending.get(response.id);
    if (pending === undefined) return;
    this.#pending.delete(response.id);
    clearTimeout(pending.timer);
    if (response.ok) {
      pending.resolve(response.result);
      return;
    }
    pending.reject(new Error(`${response.error.code}: ${response.error.message}`));
  }

  #restart(reason: string): void {
    this.#worker.terminate();
    for (const pending of this.#pending.values()) {
      clearTimeout(pending.timer);
      pending.reject(new Error(`PGW0002: ${reason}`));
    }
    this.#pending.clear();
    this.#worker = this.#spawn();
  }
}
