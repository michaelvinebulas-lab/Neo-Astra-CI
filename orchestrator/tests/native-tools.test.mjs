import test from "node:test";
import assert from "node:assert/strict";
import { execFileSync, spawn, spawnSync } from "node:child_process";
import { mkdtemp, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..", "..");
const suffix = process.platform === "win32" ? ".exe" : "";

execFileSync("cargo", ["build", "-q", "-p", "astra-cli", "-p", "astra-lsp"], {
  cwd: root,
  stdio: "inherit",
});

function cli(args) {
  return spawnSync(join(root, "target", "debug", `astra-native${suffix}`), args, {
    cwd: root,
    encoding: "utf8",
  });
}

test("native CLI checks a valid and an invalid file", async () => {
  const directory = await mkdtemp(join(tmpdir(), "neo-astra-native-"));
  try {
    const valid = join(directory, "valid.ns");
    const broken = join(directory, "broken.ns");
    await writeFile(valid, "pub fn main() -> i32 { return 0 }\n", "utf8");
    await writeFile(broken, 'pub fn main() -> i32 { return "no" }\n', "utf8");

    const good = cli(["check", valid]);
    assert.equal(good.status, 0, good.stderr);
    assert.equal(good.stderr, "");

    const bad = cli(["check", broken, "--json"]);
    assert.equal(bad.status, 1);
    assert.match(bad.stdout, /"NS4001"/);
    assert.equal(bad.stderr, "");
  } finally {
    await rm(directory, { recursive: true, force: true });
  }
});

function parseFrames(buffer) {
  const messages = [];
  let offset = 0;
  while (true) {
    const headerEnd = buffer.indexOf("\r\n\r\n", offset, "utf8");
    if (headerEnd < 0) break;
    const header = buffer.slice(offset, headerEnd);
    const match = /content-length:\s*(\d+)/i.exec(header);
    if (!match) break;
    const length = Number(match[1]);
    const bodyStart = headerEnd + 4;
    if (Buffer.byteLength(buffer.slice(bodyStart), "utf8") < length) break;
    const body = buffer.slice(bodyStart, bodyStart + length);
    messages.push(JSON.parse(body.toString("utf8")));
    offset = bodyStart + length;
  }
  return messages;
}

test("LSP publishes diagnostics and initialize capabilities", async () => {
  const server = spawn(join(root, "target", "debug", `astra-lsp${suffix}`), [], {
    cwd: root,
    stdio: ["pipe", "pipe", "pipe"],
  });
  let output = "";
  const received = [];
  const waiters = [];
  server.stdout.on("data", (chunk) => {
    output += chunk.toString("utf8");
    received.splice(0, received.length, ...parseFrames(output));
    for (const waiter of [...waiters]) {
      const found = received.find((message) => waiter.predicate(message));
      if (found) {
        waiters.splice(waiters.indexOf(waiter), 1);
        waiter.resolve(found);
      }
    }
  });

  const waitFor = (predicate) =>
    new Promise((resolve, reject) => {
      for (const message of received) {
        const found = predicate(message);
        if (found) {
          resolve(found);
          return;
        }
      }
      const timer = setTimeout(() => reject(new Error(`LSP timeout:\n${output}`)), 10_000);
      waiters.push({
        predicate,
        resolve: (value) => {
          clearTimeout(timer);
          resolve(value);
        },
      });
    });

  const send = (message) => {
    const body = JSON.stringify(message);
    server.stdin.write(`Content-Length: ${Buffer.byteLength(body)}\r\n\r\n${body}`);
  };

  try {
    send({
      jsonrpc: "2.0",
      id: 1,
      method: "initialize",
      params: { processId: null, rootUri: null, capabilities: {} },
    });
    const initialize = await waitFor((message) => message.id === 1);
    assert.equal(initialize.result.capabilities.textDocumentSync, 1);
    assert.equal(initialize.result.capabilities.documentFormattingProvider, true);

    send({
      jsonrpc: "2.0",
      method: "textDocument/didOpen",
      params: {
        textDocument: {
          uri: "file:///broken.ns",
          languageId: "neoa",
          version: 1,
          text: 'pub fn main() -> i32 { return "no" }\n',
        },
      },
    });
    const diagnostics = await waitFor(
      (message) =>
        message.method === "textDocument/publishDiagnostics" &&
        message.params.uri === "file:///broken.ns",
    );
    assert.ok(
      diagnostics.params.diagnostics.some((item) => item.code === "NS4001"),
      JSON.stringify(diagnostics),
    );
    assert.equal(diagnostics.params.diagnostics[0].source, "astra");
  } finally {
    server.kill();
  }
});
