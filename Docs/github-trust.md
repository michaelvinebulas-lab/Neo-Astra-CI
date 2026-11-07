# GitHub trust state

This is an internal operations note. Public policy lives in `public/SECURITY.md`
and `public/CONTRIBUTING.md`.

## Verified

- Public `master` is the sanitized mirror at
  `17ceaf66a094d6e22a66302e480d276abd5ddfd8` (docs + native phase-1 crates).
- `v1.0.0` still points at annotated tag object
  `56adfc688715f0c702b29f1e9e24766525b2857e` -> `9d832e0`.
- Release assets and `.sha256` sidecars are present for Windows x64, Linux x64,
  and macOS arm64.
- The public release workflow requests `contents: write`, `id-token: write`,
  and `attestations: write`; it attests archives and publishes npm packages
  with `--provenance`.
- The 1.0.0 npm tarballs have registry signatures but no npm provenance
  attestation. The 1.0.0 GitHub assets also return 404 from the attestation
  API. The provenance workflow landed after the tag, so do not claim
  provenance for 1.0.0; verify it on 1.0.1 and later.
- `public/` carries `LICENSE`, `NOTICE`, `CONTRIBUTORS.md`, `SECURITY.md`,
  `CONTRIBUTING.md`, `CODEOWNERS`, issue templates, and the support matrix.
- The generated mirror is formatted, compiles, and is clippy-clean with
  `cargo fmt --all --check` and
  `cargo clippy --workspace --all-targets -- -D warnings`.

## Still missing

These require the repository owner's GitHub or npm credentials.

- No repository rulesets exist yet:
  `GET /repos/luminousvalentine-cell/Neo-Astra/rulesets` returns `[]`.
- The v1.0.0 GitHub release reports `immutable: false`. Treat the tag as
  immutable by policy; do not modify its assets. Immutability for future
  releases must be enabled in repository settings, if available.
- The npm token exposed during the 1.0.0 release still needs to be revoked.
  The release workflow no longer reads `NPM_TOKEN`; publish authentication is
  OIDC trusted publishing.
- npm trusted publishing still needs to be configured for the platform
  packages and the `neo-astra` meta package.
- No public CI workflow exists yet. Do not require status checks in a ruleset
  until the CI workflow and check names exist.

## npm trusted publishing

The release workflow now:

- requests `id-token: write`;
- installs npm 11 (trusted publishing requires npm 11.5.1 or later);
- sets `registry-url` and disables package-manager caching;
- publishes with `--provenance` and no `NODE_AUTH_TOKEN`.

After authenticating as an account that can administer all four packages, add
one trusted relationship per package:

```sh
npm login
for package in \
  neo-astra \
  neo-astra-windows-x64 \
  neo-astra-linux-x64 \
  neo-astra-darwin-arm64; do
  npm trust github "$package" \
    --file release.yml \
    --repository luminousvalentine-cell/Neo-Astra \
    --yes
done
npm trust list neo-astra
```

The workflow filename must stay exactly `release.yml`, and the repository must
stay exactly `luminousvalentine-cell/Neo-Astra`. Do not set an environment
unless the release jobs add a matching GitHub environment.

After the first successful OIDC publish, delete the `NPM_TOKEN` secret. Keep it
until the trusted publisher is verified so a rollback is possible.

## Rulesets

Create these after authenticating `gh` as an owner with ruleset permission:

```sh
gh auth login
gh api --method POST repos/luminousvalentine-cell/Neo-Astra/rulesets --input branch-ruleset.json
gh api --method POST repos/luminousvalentine-cell/Neo-Astra/rulesets --input tag-ruleset.json
```

`branch-ruleset.json`:

```json
{
  "name": "protect-master",
  "target": "branch",
  "enforcement": "active",
  "conditions": {
    "ref_name": {
      "include": ["~DEFAULT_BRANCH"],
      "exclude": []
    }
  },
  "rules": [
    { "type": "deletion" },
    { "type": "non_fast_forward" },
    { "type": "required_linear_history" },
    {
      "type": "pull_request",
      "parameters": {
        "required_approving_review_count": 0,
        "dismiss_stale_reviews_on_push": true,
        "require_code_owner_review": true,
        "require_last_push_approval": false,
        "required_review_thread_resolution": true
      }
    }
  ]
}
```

The approval count is deliberately `0` while the repository has one
maintainer. Raise it to `1` when a second reviewer exists. Add a
`required_status_checks` rule only after Kilo's CI slice lands and the check
names are known.

`tag-ruleset.json`:

```json
{
  "name": "immutable-release-tags",
  "target": "tag",
  "enforcement": "active",
  "conditions": {
    "ref_name": {
      "include": ["refs/tags/v*"],
      "exclude": []
    }
  },
  "rules": [
    { "type": "deletion" },
    { "type": "update" }
  ]
}
```

## Source mirror flow

The local pre-push hook currently allows only:

```text
refs/heads/source -> refs/heads/master
```

That is compatible with an unprotected `master`, not with a ruleset that
requires pull requests. Before enabling the branch ruleset, choose one of:

1. Add a ruleset bypass for a dedicated release identity, and keep the
   owner-reviewed path for human changes.
2. Change the mirror flow to push `source` to a short-lived branch, open a pull
   request, and merge it with checks.
3. Keep the owner as the only bypass actor and record that exception in
   `public/CONTRIBUTING.md` (not recommended; the public text says direct
   pushes are blocked).

Do not enable the branch ruleset until the mirror flow is changed or a bypass
is deliberate.

## Release verification

After a release, verify at least:

```sh
gh attestation verify dist/neo-astra-1.0.1-windows-x64.zip \
  --repo luminousvalentine-cell/Neo-Astra
npm audit signatures
npm view neo-astra@1.0.1 dist.attestations
```

For npm, `npm audit signatures` must run in a clean install directory so it
checks the published registry metadata. A missing attestation is a release
failure, not a warning.
