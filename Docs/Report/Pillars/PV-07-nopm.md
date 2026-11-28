# PV-07 — NoPM

**Status:** Partial
**Pillar:** One lockfile across path, Git, npm, Cargo, PyPI, NuGet, and OCI; no required first-party registry.

## Original promise

> NoPM resolves path, Git, npm, and Cargo packages into a deterministic `core.lock.json`.
> — `Docs/archive/Design/V1/README.md` release criteria

> Local development must work without the gateway.
> — `Docs/archive/architecture.md` §8.6.1

## What we can currently show

- Package IDs, descriptors, target/mode filtering, deterministic graph, lockfile I/O, content-addressed store.
- Adapters for path, Git, npm, Cargo, PyPI, NuGet, OCI, and `sys`.
- CLI: `lock`, `tree`, `install`, `search`, `audit`, `why`, `update`.
- `nopm publish` is missing; registry gateway and target-scoped resolution need an end-to-end proof.
- The playground plan (`1.2`) proves the browser-side resolver and lockfile shape.

## Proof program

`conformance/vision/pv07-project/core.json`:

```json
{
  "package": { "name": "pv07-app", "version": "1.0.0", "edition": "0.1" },
  "dependencies": {
    "local": { "path": "./packages/local" },
    "npm:is-odd": { "version": "3.0.1", "targets": ["web"] },
    "cargo:itoa": { "version": "1.0", "targets": ["native"] }
  }
}
```

`conformance/vision/pv07-project/src/main.ns` imports from all three and prints one value from each.

## Gate

1. `nopm install --locked` twice from a clean cache produces a byte-identical `core.lock.json`.
2. Web build includes only the npm dependency; native build includes only the Cargo dependency.
3. Removing a call changes the resolved variant and CI shows the diff.
4. Integrity hashes are verified; lifecycle scripts remain disabled by default.
5. `nopm publish` round-trip: a package can be packed, published, installed, and locked by a second project.

## Work implied

1. Implement `nopm publish` or narrow the public claim.
2. Add a target-scoped dependency fixture to conformance.
3. Keep the optional gateway optional — no test may require it.

## Task list

- [ ] Implement or explicitly narrow the public claim for `nopm publish`.
- [ ] Add the target-scoped dependency fixture with path, npm, and Cargo sources.
- [ ] Verify `nopm install --locked` twice from a clean cache produces an identical lockfile.
- [ ] Verify web includes only the npm dependency and native only the Cargo dependency.
- [ ] Verify integrity hashes and lifecycle-script policy.
- [ ] Verify no proof requires the optional gateway.
- [ ] Add a publish/install/lock round-trip with a second project.
- [ ] Record evidence and change status to `Proven`.

## Release code

`PV-07`. Release target: TBD.
