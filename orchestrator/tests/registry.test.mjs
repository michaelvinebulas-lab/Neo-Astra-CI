import assert from "node:assert/strict";
import test from "node:test";

import {
  RegistryError,
  RegistryStore,
  generateRegistryKeyPair,
  signRegistryPackage,
  startRegistryServer,
  verifyRegistryPackage,
} from "../packages/registry/dist/index.js";

function publish(store, version = "1.0.0") {
  return store.publish({
    name: "@scope/pkg",
    version,
    dependencies: { lib: "npm:lib@2" },
    targets: ["web"],
    capabilities: ["dom"],
    integrity: "sha512-pkg",
    resolved: `https://registry.example/pkg-${version}.tgz`,
  });
}

test("registry store publishes, searches, yanks, and audits", () => {
  const store = new RegistryStore();
  const published = publish(store, "1.0.0");
  publish(store, "1.1.0");

  assert.equal(published.version, "1.0.0");
  assert.equal(store.get("@scope/pkg", "1.0.0").integrity, "sha512-pkg");
  assert.deepEqual(
    store.versions("@scope/pkg").map((entry) => entry.version),
    ["1.0.0", "1.1.0"],
  );
  assert.deepEqual(store.search("pkg").map((entry) => entry.name), ["@scope/pkg"]);

  const yanked = store.yank("@scope/pkg", "1.0.0");
  assert.equal(yanked.yanked, true);
  assert.deepEqual(store.audit("@scope/pkg", "1.0.0"), {
    name: "@scope/pkg",
    version: "1.0.0",
    yanked: true,
    advisories: [],
  });

  assert.throws(
    () => publish(store, "1.0.0"),
    (error) => error instanceof RegistryError && error.code === "NR0001",
  );
  assert.throws(
    () =>
      store.publish({
        name: "bad",
        version: "1",
        integrity: "",
        resolved: "",
      }),
    (error) => error instanceof RegistryError && error.code === "NR0002",
  );
  assert.throws(
    () => store.get("@scope/pkg", "9.9.9"),
    (error) => error instanceof RegistryError && error.code === "NR0003",
  );
});

test("registry HTTP API serves, publishes, searches, and yanks", async () => {
  const store = new RegistryStore();
  const { server, url } = await startRegistryServer(store);
  try {
    const publishResponse = await fetch(`${url}/v1/publish`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({
        name: "@scope/pkg",
        version: "1.0.0",
        dependencies: { lib: "npm:lib@2" },
        targets: ["web"],
        capabilities: ["dom"],
        integrity: "sha512-pkg",
        resolved: "https://registry.example/pkg.tgz",
      }),
    });
    assert.equal(publishResponse.status, 201);

    const name = encodeURIComponent("@scope/pkg");
    const getResponse = await fetch(`${url}/v1/packages/${name}/1.0.0`);
    assert.equal(getResponse.status, 200);
    const descriptor = await getResponse.json();
    assert.equal(descriptor.version, "1.0.0");
    assert.equal(descriptor.integrity, "sha512-pkg");

    const versionsResponse = await fetch(`${url}/v1/packages/${name}`);
    assert.equal(versionsResponse.status, 200);
    const versions = await versionsResponse.json();
    assert.deepEqual(versions.versions, [{ version: "1.0.0", yanked: false }]);

    const searchResponse = await fetch(`${url}/v1/search?q=pkg`);
    const search = await searchResponse.json();
    assert.equal(search.results.length, 1);

    const yankResponse = await fetch(`${url}/v1/yank`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ name: "@scope/pkg", version: "1.0.0" }),
    });
    assert.equal(yankResponse.status, 200);

    const yankedGet = await fetch(`${url}/v1/packages/${name}/1.0.0`);
    assert.equal(yankedGet.status, 410);

    const auditResponse = await fetch(`${url}/v1/audit/${name}/1.0.0`);
    assert.equal(auditResponse.status, 200);
    const audit = await auditResponse.json();
    assert.equal(audit.yanked, true);
  } finally {
    await new Promise((resolve) => server.close(resolve));
  }
});

test("registry signatures verify and are accepted by a signed server", async () => {
  const keys = generateRegistryKeyPair();
  const request = {
    name: "@scope/signed",
    version: "1.0.0",
    dependencies: { lib: "npm:lib@2" },
    targets: ["web"],
    capabilities: [],
    integrity: "sha512-signed",
    resolved: "https://registry.example/signed.tgz",
  };
  const signature = signRegistryPackage(request, keys.privateKey);
  assert.equal(verifyRegistryPackage(request, signature, keys.publicKey), true);
  assert.equal(
    verifyRegistryPackage({ ...request, resolved: "https://evil.example/x" }, signature, keys.publicKey),
    false,
  );

  const store = new RegistryStore();
  const { server, url } = await startRegistryServer(store, 0, "127.0.0.1", {
    requireSignature: true,
    trustedPublicKey: keys.publicKey,
  });
  try {
    const signedResponse = await fetch(`${url}/v1/publish`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({
        ...request,
        signature,
        publicKey: keys.publicKey,
      }),
    });
    assert.equal(signedResponse.status, 201);

    const unsignedResponse = await fetch(`${url}/v1/publish`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({
        ...request,
        name: "@scope/unsigned",
      }),
    });
    assert.equal(unsignedResponse.status, 401);
  } finally {
    await new Promise((resolve) => server.close(resolve));
  }
});

test("registry can mirror a missing package from an upstream gateway", async () => {
  const upstreamStore = new RegistryStore();
  publish(upstreamStore, "2.0.0");
  const upstream = await startRegistryServer(upstreamStore);
  const downstreamStore = new RegistryStore();
  const downstream = await startRegistryServer(downstreamStore, 0, "127.0.0.1", {
    upstreams: [upstream.url],
  });
  try {
    const name = encodeURIComponent("@scope/pkg");
    const response = await fetch(`${downstream.url}/v1/packages/${name}/2.0.0`);
    assert.equal(response.status, 200);
    const descriptor = await response.json();
    assert.equal(descriptor.version, "2.0.0");
    assert.equal(downstreamStore.get("@scope/pkg", "2.0.0").integrity, "sha512-pkg");
  } finally {
    await new Promise((resolve) => downstream.server.close(resolve));
    await new Promise((resolve) => upstream.server.close(resolve));
  }
});

