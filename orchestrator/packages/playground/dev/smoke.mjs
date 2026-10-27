// Local smoke test for the playground bridge. Not part of the source snapshot.
// Run: node orchestrator/packages/playground/dev/smoke.mjs
import { readFile } from "node:fs/promises";
import { CompilerBridge } from "../src/bridge.ts";

const wasmPath =
  process.env.ASTRA_WASM ??
  "C:/Projects/Neo Astra/target/wasm32-unknown-unknown/debug/astra_wasm.wasm";

const bridge = await CompilerBridge.instantiate(await readFile(wasmPath));
console.log("protocol:", bridge.protocolVersion);

const check = {
  jsonrpc: "2.0",
  id: 1,
  method: "check",
  params: {
    apiVersion: "1.0",
    source: {
      path: "src/main.ns",
      text: "module app.main\npub fn add(a: i32, b: i32) -> i32 {\n    return a + b\n}\n",
    },
    target: "js",
    mode: "core",
  },
};

const result = await bridge.request(check);
console.log("check:", JSON.stringify(result));

try {
  await bridge.request({
    ...check,
    id: 2,
    params: {
      ...check.params,
      source: { path: "src/main.ns", text: "private fn secret() {}\nexport { secret }\n" },
    },
  });
  console.log("expected an NS3001 diagnostic");
} catch (error) {
  console.log("error class:", error.constructor.name);
  console.log("diagnostics:", JSON.stringify(error.message).slice(0, 200));
}

bridge.close();
console.log("closed:", bridge.closed);
