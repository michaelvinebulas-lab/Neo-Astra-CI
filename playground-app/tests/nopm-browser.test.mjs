import assert from "node:assert/strict";
import test from "node:test";

import { dependencySpec, resolvePackages } from "../dist/nopm-browser.js";

test("dependencySpec accepts direct specs", () => {
  assert.equal(dependencySpec("odd", "npm:is-odd@3.0.1"), "npm:is-odd@3.0.1");
});

test("dependencySpec accepts object forms", () => {
  assert.equal(dependencySpec("util", { path: "./packages/util" }), "path:./packages/util");
  assert.equal(dependencySpec("itoa", { source: "cargo", version: "1.0" }), "cargo:itoa@1.0");
  assert.equal(dependencySpec("bad", {}), "");
});

test("resolvePackages produces a standard lockfile", async () => {
  const files = new Map([["core.json", '{"package":{"name":"hello","version":"1.0.0"}}']]);
  const result = await resolvePackages(files, {
    apiVersion: "1.0",
    edition: "0.1",
    target: "js",
    mode: "core",
    integerPolicy: "wrapping",
  });
  const parsed = JSON.parse(result.lockfile);
  assert.equal(parsed.lockfileVersion, 1);
  assert.equal(parsed.generatedBy, "neo-astra-playground");
  assert.deepEqual(parsed.packages, []);
  assert.deepEqual(result.issues, []);
});
