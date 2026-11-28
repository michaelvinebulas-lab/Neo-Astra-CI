import assert from "node:assert/strict";
import test from "node:test";

import { projectSettings, validateManifestFiles } from "../dist/manifest.js";

function validate(files) {
  return validateManifestFiles(new Map(Object.entries(files)));
}

function expectOk(files) {
  const result = validate(files);
  assert.equal(result.ok, true, JSON.stringify(result.issues));
  assert.deepEqual(result.issues, []);
}

function expectIssue(files, code) {
  const result = validate(files);
  assert.equal(result.ok, false);
  assert.ok(
    result.issues.some((issue) => issue.code === code),
    `expected ${code}, got ${JSON.stringify(result.issues)}`,
  );
}

const coreJson = '{"package":{"name":"hello","version":"1.0.0","edition":"0.1"}}';
const coreToml = '[package]\nname = "hello"\nversion = "1.0.0"\nedition = "0.1"\n';
const buildJson =
  '{"build":{"defaultTarget":"js"},"math":{"integers":"wrapping"},"strings":{"model":"utf8"}}';

test("1. valid core.json", () => expectOk({ "core.json": coreJson }));
test("2. valid core.toml", () => expectOk({ "core.toml": coreToml }));
test("3. build key in core.json is rejected", () =>
  expectIssue({ "core.json": '{"package":{"name":"x"},"math":{"integers":"wrapping"}}' }, "NS9003"));
test("4. core key in build.toml is rejected", () =>
  expectIssue({ "core.json": coreJson, "build.toml": '[package]\nname = "x"\n' }, "NS9003"));
test("5. duplicate JSON keys are rejected", () =>
  expectIssue({ "core.json": '{"package":{"name":"a"},"package":{"name":"b"}}' }, "NS9002"));
test("6. both core manifests are rejected", () =>
  expectIssue({ "core.json": coreJson, "core.toml": coreToml }, "NS9002"));
test("7. invalid JSON is rejected", () =>
  expectIssue({ "core.json": "{ not json" }, "NS9002"));
test("8. invalid TOML is rejected", () =>
  expectIssue({ "core.json": coreJson, "build.toml": "math = [unterminated\n" }, "NS9002"));
test("9. unknown key is rejected", () =>
  expectIssue({ "core.json": '{"package":{"name":"x"},"surprise":true}' }, "NS9002"));
test("10. valid build.json with math and strings", () =>
  expectOk({ "core.json": coreJson, "build.json": buildJson }));
test("11. missing core manifest is reported", () =>
  expectIssue({ "src/main.ns": "pub fn main() {}" }, "NS9001"));
test("12. invalid lockfile is reported", () =>
  expectIssue({ "core.json": coreJson, "core.lock.json": '{"version":1}' }, "NS9002"));

function settingsFor(files) {
  return projectSettings(validateManifestFiles(new Map(Object.entries(files))));
}

test("13. project settings default to js and core", () => {
  const settings = settingsFor({ "core.json": coreJson });
  assert.equal(settings.edition, "0.1");
  assert.equal(settings.target, "js");
  assert.equal(settings.mode, undefined);
  assert.equal(settings.integerPolicy, undefined);
});

test("14. project settings read the manifests", () => {
  const settings = settingsFor({ "core.json": coreJson, "build.json": buildJson });
  assert.equal(settings.target, "js");
  assert.equal(settings.integerPolicy, "wrapping");
});
