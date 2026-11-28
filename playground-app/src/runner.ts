/** Sandboxed execution of generated JavaScript modules. */

import type { CompiledFile } from "./codegen.js";

export interface RunResult {
  readonly output: string;
  readonly passed: boolean | undefined;
}

interface RunnerMessage {
  readonly source?: unknown;
  readonly type?: unknown;
  readonly text?: unknown;
}

function specifier(key: string): string {
  return `@playground/${encodeURIComponent(key)}`;
}

function dataUrl(code: string): string {
  return `data:text/javascript,${encodeURIComponent(code)}`;
}

export function runProject(
  files: readonly CompiledFile[],
  expected: string,
  timeoutMs = 10000,
): Promise<RunResult> {
  return new Promise<RunResult>((resolve, reject) => {
    if (files.length === 0) {
      reject(new Error("Nothing to run. Compile the project first."));
      return;
    }

    const imports = Object.fromEntries(files.map((file) => [specifier(file.key), dataUrl(file.code)]));
    const entry =
      files.find((file) => file.key.includes("main"))?.key ?? (files[0] as CompiledFile).key;
    const html = `<!doctype html>
<html><body>
<script>
globalThis.__pgSend = function (type, text) {
  parent.postMessage({ source: "playground-runner", type: type, text: String(text ?? "") }, "*");
};
globalThis.process = {
  stdout: { write: function (text) { globalThis.__pgSend("stdout", text); } },
  stderr: { write: function (text) { globalThis.__pgSend("stderr", text); } },
  exit: function (code) { globalThis.__pgSend("exit", code); }
};
</script>
<script type="importmap">${JSON.stringify({ imports })}</script>
<script type="module">
import(${JSON.stringify(specifier(entry))})
  .then(function () { globalThis.__pgSend("done", ""); })
  .catch(function (error) { globalThis.__pgSend("error", error && error.stack ? error.stack : String(error)); });
</script>
</body></html>`;

    const iframe = document.createElement("iframe");
    iframe.setAttribute("sandbox", "allow-scripts");
    iframe.style.display = "none";

    let output = "";
    let settled = false;
    const finish = (result: RunResult | undefined, error?: Error): void => {
      if (settled) return;
      settled = true;
      clearTimeout(timer);
      window.removeEventListener("message", handler);
      iframe.remove();
      if (error !== undefined) reject(error);
      else resolve(result as RunResult);
    };
    const timer = setTimeout(() => {
      finish(undefined, new Error("PGW0002: program exceeded the run time limit."));
    }, timeoutMs);
    const handler = (event: MessageEvent<RunnerMessage>): void => {
      if (event.source !== iframe.contentWindow) return;
      if (event.origin !== "null" && event.origin !== window.location.origin) return;
      const message = event.data;
      if (message === null || typeof message !== "object" || message["source"] !== "playground-runner") {
        return;
      }
      const type = message["type"];
      const text = String(message["text"] ?? "");
      if (type === "stdout" || type === "stderr") {
        output += text;
        return;
      }
      if (type === "error") {
        finish(undefined, new Error(text));
        return;
      }
      const trimmed = expected.trim();
      finish({
        output,
        passed: trimmed === "" ? undefined : output.trim() === trimmed,
      });
    };

    window.addEventListener("message", handler);
    iframe.srcdoc = html;
    document.body.append(iframe);
  });
}
