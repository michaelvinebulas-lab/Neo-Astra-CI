import assert from "node:assert/strict";
import test from "node:test";

import { DEFAULT_OVERRIDES, resolveSettings, withOptionalFields } from "../dist/settings.js";

const manifest = {
  edition: "0.1",
  target: "js",
  mode: undefined,
  integerPolicy: "wrapping",
};

test("settings use manifest values on auto", () => {
  const settings = resolveSettings(manifest, DEFAULT_OVERRIDES);
  assert.equal(settings.edition, "0.1");
  assert.equal(settings.target, "js");
  assert.equal(settings.mode, undefined);
  assert.equal(settings.integerPolicy, "wrapping");
});

test("settings apply UI overrides", () => {
  const settings = resolveSettings(manifest, {
    edition: "0.2",
    target: "ts",
    mode: "web",
    integerPolicy: "safe-checked",
  });
  assert.equal(settings.edition, "0.2");
  assert.equal(settings.target, "ts");
  assert.equal(settings.mode, "web");
  assert.equal(settings.integerPolicy, "safe-checked");
  const wire = withOptionalFields(settings);
  assert.equal(wire["edition"], "0.2");
  assert.equal(wire["mode"], "web");
});

test("optional fields are omitted when unset", () => {
  const wire = withOptionalFields(resolveSettings(manifest, DEFAULT_OVERRIDES));
  assert.equal(Object.hasOwn(wire, "mode"), false);
});
