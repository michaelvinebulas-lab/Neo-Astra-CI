/**
 * Browser-safe bridge to the Neo Astra compiler core compiled to WebAssembly.
 *
 * The Rust core exposes a raw linear-memory ABI (see `crates/astra-wasm`):
 *
 * - `astra_protocol_version() -> u32`
 * - `astra_alloc(len) -> ptr`
 * - `astra_free(ptr, len)`
 * - `astra_compile(ptr, len) -> u64` where the high 32 bits are the response
 *   pointer and the low 32 bits are the response length.
 *
 * This module owns every detail of that ABI: allocation, UTF-8 framing, the
 * packed response, freeing both buffers, and turning traps into stable errors.
 * Callers only deal in JSON-RPC request and response objects.
 *
 * `memory.buffer` is re-read on every access because a growing Wasm memory
 * detaches previously created views.
 */

/** Protocol version this host was written against. */
export const SUPPORTED_PROTOCOL_VERSION = 1;

/** Stable playground bridge error codes. They are not compiler diagnostics. */
export type BridgeErrorCode =
  | "PG0001" // protocol version mismatch
  | "PG0002" // Wasm trap or panic inside the compiler
  | "PG0003" // malformed or truncated compiler response
  | "PG0004"; // allocation failure inside the Wasm instance

/** A bridge-level failure, never a user syntax error. */
export class CompilerBridgeError extends Error {
  /** Stable machine-readable code. */
  readonly code: BridgeErrorCode;

  constructor(code: BridgeErrorCode, message: string, options?: { cause?: unknown }) {
    super(message, options as ErrorOptions | undefined);
    this.name = "CompilerBridgeError";
    this.code = code;
  }
}

/** A JSON-RPC error returned by the compiler itself. */
export class CompilerResponseError extends Error {
  /** JSON-RPC error code. */
  readonly code: number;
  /** Optional structured error payload. */
  readonly data: unknown;

  constructor(code: number, message: string, data?: unknown) {
    super(message);
    this.name = "CompilerResponseError";
    this.code = code;
    this.data = data;
  }
}

export interface JsonRpcRequest<Params = unknown> {
  jsonrpc: "2.0";
  id: number | string;
  method: string;
  params: Params;
}

export interface JsonRpcSuccess<Result = unknown> {
  jsonrpc: "2.0";
  id: number | string;
  result: Result;
}

export interface JsonRpcFailure {
  jsonrpc: "2.0";
  id: number | string | null;
  error: {
    code: number;
    message: string;
    data?: unknown;
  };
}

export type JsonRpcResponse<Result = unknown> = JsonRpcSuccess<Result> | JsonRpcFailure;

/** The exports the bridge depends on. Anything missing is a build error. */
interface CompilerWasmExports {
  readonly memory: WebAssembly.Memory;
  astra_protocol_version(): number;
  astra_alloc(len: number): number;
  astra_free(ptr: number, len: number): void;
  astra_compile(ptr: number, len: number): bigint;
}

export interface CompilerBridgeOptions {
  /**
   * Protocol version to require. Defaults to {@link SUPPORTED_PROTOCOL_VERSION}.
   * A mismatch fails fast instead of producing silent misparses.
   */
  expectedProtocolVersion?: number;
}

const REQUIRED_EXPORTS = [
  "memory",
  "astra_protocol_version",
  "astra_alloc",
  "astra_free",
  "astra_compile",
] as const;

function readExports(exports: WebAssembly.Exports): CompilerWasmExports {
  for (const name of REQUIRED_EXPORTS) {
    if (!(name in exports)) {
      throw new CompilerBridgeError(
        "PG0003",
        `Wasm module is missing the '${name}' export; rebuild crates/astra-wasm`,
      );
    }
  }

  return exports as unknown as CompilerWasmExports;
}

/** Owns one instantiated compiler. */
export class CompilerBridge {
  readonly #exports: CompilerWasmExports;
  readonly #encoder = new TextEncoder();
  readonly #decoder = new TextDecoder("utf-8", { fatal: true });
  #closed = false;

  private constructor(exports: CompilerWasmExports) {
    this.#exports = exports;
  }

  /**
   * Compile and instantiate the Wasm module, then verify its protocol version.
   *
   * `source` accepts bytes or an already-compiled `WebAssembly.Module`, so a
   * host can cache compilation across reloads.
   */
  static async instantiate(
    source: BufferSource | WebAssembly.Module,
    options: CompilerBridgeOptions = {},
  ): Promise<CompilerBridge> {
    let module: WebAssembly.Module;
    try {
      module =
        source instanceof WebAssembly.Module ? source : await WebAssembly.compile(source);
    } catch (cause) {
      throw new CompilerBridgeError("PG0002", "Failed to compile the compiler Wasm module", {
        cause,
      });
    }

    let instance: WebAssembly.Instance;
    try {
      instance = await WebAssembly.instantiate(module);
    } catch (cause) {
      throw new CompilerBridgeError("PG0002", "Failed to instantiate the compiler Wasm module", {
        cause,
      });
    }

    const exports = readExports(instance.exports);
    const expected = options.expectedProtocolVersion ?? SUPPORTED_PROTOCOL_VERSION;
    const actual = exports.astra_protocol_version();
    if (actual !== expected) {
      throw new CompilerBridgeError(
        "PG0001",
        `Compiler protocol version ${actual} does not match host version ${expected}`,
      );
    }

    return new CompilerBridge(exports);
  }

  /** Protocol version reported by the loaded module. */
  get protocolVersion(): number {
    return this.#exports.astra_protocol_version();
  }

  /** Whether {@link close} has been called. */
  get closed(): boolean {
    return this.#closed;
  }

  /**
   * Send one JSON-RPC request and return its result.
   *
   * Throws {@link CompilerResponseError} for a JSON-RPC error response and
   * {@link CompilerBridgeError} for a bridge failure.
   *
   * This call is synchronous inside the Wasm instance, so it cannot be
   * pre-empted. Hard time limits belong to the Worker transport that will run
   * this bridge off the UI thread; do not fake one here.
   */
  async request<Result = unknown, Params = unknown>(
    request: JsonRpcRequest<Params>,
  ): Promise<Result> {
    const text = JSON.stringify(request);
    const raw = this.#exchange(text);
    const response = this.#parse(raw);

    if ("error" in response) {
      throw new CompilerResponseError(
        response.error.code,
        response.error.message,
        response.error.data,
      );
    }

    if (response.id !== request.id) {
      throw new CompilerBridgeError(
        "PG0003",
        `Compiler answered request '${String(response.id)}' while '${String(request.id)}' was pending`,
      );
    }

    return response.result as Result;
  }

  /** Stop accepting requests. The Wasm instance is released with the bridge. */
  close(): void {
    this.#closed = true;
  }

  #exchange(text: string): string {
    if (this.#closed) {
      throw new CompilerBridgeError("PG0003", "Compiler bridge is closed");
    }

    const input = this.#encoder.encode(text);
    const { astra_alloc, astra_free, astra_compile, memory } = this.#exports;

    let inputPointer = 0;
    try {
      if (input.byteLength > 0) {
        inputPointer = astra_alloc(input.byteLength);
        if (inputPointer === 0) {
          throw new CompilerBridgeError("PG0004", "astra_alloc returned a null pointer");
        }
        // Re-read the buffer after alloc: a growing memory detaches old views.
        new Uint8Array(memory.buffer, inputPointer, input.byteLength).set(input);
      }

      let packed: bigint;
      try {
        packed = BigInt(astra_compile(inputPointer, input.byteLength));
      } catch (cause) {
        throw new CompilerBridgeError("PG0002", "The compiler trapped while handling a request", {
          cause,
        });
      }

      const outputPointer = Number(packed >> 32n);
      const outputLength = Number(packed & 0xffff_ffffn);

      if (outputPointer === 0 || outputLength === 0) {
        throw new CompilerBridgeError("PG0003", "The compiler returned an empty response");
      }

      const buffer = memory.buffer;
      if (outputPointer + outputLength > buffer.byteLength) {
        throw new CompilerBridgeError(
          "PG0003",
          "The compiler response points outside linear memory",
        );
      }

      try {
        return this.#decoder.decode(new Uint8Array(buffer, outputPointer, outputLength));
      } catch (cause) {
        throw new CompilerBridgeError("PG0003", "The compiler response is not valid UTF-8", {
          cause,
        });
      } finally {
        astra_free(outputPointer, outputLength);
      }
    } finally {
      if (inputPointer !== 0 && input.byteLength > 0) {
        astra_free(inputPointer, input.byteLength);
      }
    }
  }

  #parse(text: string): JsonRpcResponse {
    let value: unknown;
    try {
      value = JSON.parse(text);
    } catch (cause) {
      throw new CompilerBridgeError("PG0003", "The compiler response is not valid JSON", {
        cause,
      });
    }

    if (
      typeof value !== "object" ||
      value === null ||
      (value as { jsonrpc?: unknown }).jsonrpc !== "2.0"
    ) {
      throw new CompilerBridgeError("PG0003", "The compiler response is not JSON-RPC 2.0");
    }

    const response = value as JsonRpcResponse;
    if ("error" in response) {
      const error = response.error;
      if (typeof error !== "object" || error === null || typeof error.code !== "number") {
        throw new CompilerBridgeError("PG0003", "The compiler returned a malformed JSON-RPC error");
      }
      return response;
    }

    if (!("result" in response)) {
      throw new CompilerBridgeError("PG0003", "The compiler response has no result");
    }

    return response;
  }
}
