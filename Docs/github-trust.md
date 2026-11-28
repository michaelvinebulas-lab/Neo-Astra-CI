# GitHub trust state

This is an internal operations note. Public policy lives in `public/SECURITY.md`
and `public/CONTRIBUTING.md`.

**Last checked:** 2026-11-23. **Current release:** `v1.0.1` is tagged at public
`3093343`; the GitHub release assets and attestations are published. npm
`1.0.1` is blocked on re-creating the trusted publishers.

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
- The principal added the GitHub Actions trusted relationships for all four
  packages on 2026-11-07. npm expires unvalidated configurations after two
  days, so those relationships expired before the first successful publish.
  On 2026-11-23 the OIDC exchange returned
  `OIDC token exchange error - package not found` for every platform package.
  Re-create the relationships immediately before the next publish attempt.
- `public/` carries `LICENSE`, `NOTICE`, `CONTRIBUTORS.md`, `SECURITY.md`,
  `CONTRIBUTING.md`, `CODEOWNERS`, issue templates, and the support matrix.
- The generated mirror is formatted, compiles, and is clippy-clean with
  `cargo fmt --all --check` and
  `cargo clippy --workspace --all-targets -- -D warnings`.

## Still missing / open

These require the repository owner's GitHub or npm credentials, a completed
release, or a principal decision.

- `1.0.1` is not tagged or published. The native CLI/NoPM port is blocked at
  slice 3a; slices 3b and 4 are not started; native parity is 11/12. Do not
  publish until the release checklist is green.
- Repository rulesets are active:
  - `immutable-release-tags` blocks deletion and updates of `refs/tags/v*`.
  - `protect-master` blocks deletion, force pushes, and non-linear history on
    the default branch.
  - `protect-master` requires a pull request with resolved review threads. The
    repository currently has one maintainer, so the required approval count is
    zero and code-owner approval is not enforced; owner self-merge is allowed.
- The public `source` branch is the staging branch for mirror updates. It is
  rewritten for each snapshot and is not protected.
- The source mirror pre-push guard allows `source -> source` for PR staging and
  rejects private paths. Direct `source -> master` pushes are blocked by both
  the guard flow and the GitHub ruleset.
- The v1.0.0 GitHub release reports `immutable: false`. Treat the tag as
  immutable by policy; do not modify its assets. Immutability for future
  releases must be enabled in repository settings, if available.
- The npm token exposed during the 1.0.0 release has been revoked. The release
  workflow no longer reads `NPM_TOKEN`; publish authentication is OIDC trusted
  publishing. The recreated configs were validated by the `1.1.1` publish and
  must be re-created again before the next release.
- The trusted relationships were added but are unvalidated until the first
  successful OIDC publish. `npm trust list` currently requires passkey
  re-authentication and returns `EOTP`; record that as a principal action, not
  a bypass. Refresh the relationships immediately before the next release.
- The public LSP `serverInfo` still reports `2.0.0`; correct it before the next
  package that ships the server.
- No public CI workflow exists yet. Do not require status checks in a ruleset
  until the CI workflow and check names exist.

## npm trusted publishing

The release workflow now:

- requests `id-token: write` at the job level;
- installs npm 11 (trusted publishing requires npm 11.5.1 or later);
- does **not** set `registry-url` on `actions/setup-node@v4`, because that
  action defaults its `token` input to `github.token` and writes the GitHub
  token into `.npmrc` as the npm auth token; that caused `E404` on publish;
- publishes with `--provenance` and no `NODE_AUTH_TOKEN`.

The trust configuration itself must be created with npm **12.2.3 or later**.
Older CLIs (including 11.13.0) do not send the required `permissions` field,
so the registry rejects the request with `400`; and a config without
`createPackage` cannot publish, so the OIDC exchange reports
`package not found`.

npm's current 2FA is a security key/passkey (WebAuthn), not an
authenticator-app TOTP. Do not pass a Chrome passkey to `--otp`; it is not a
six-digit code. Use the npm website instead of the CLI: package page ->
**Settings** -> **Trusted publishing** -> **Add trusted publisher** -> **GitHub
Actions**. Use repository `luminousvalentine-cell/Neo-Astra` and workflow
filename `release.yml`, and leave the environment blank.

Make sure the configuration allows **npm publish**, not only stage publishing;
the release workflow runs `npm publish`. New configurations must complete their
first successful publish within two days or they expire and must be recreated.
For that reason, configure immediately before the release that will validate
them.

After authenticating as an account that can administer all four packages,
revoke any old relationship, then add one publish-enabled relationship per
package:

```sh
npm login
# Revoke old IDs from `npm trust list <package> --json` first.
npx -y npm@latest trust revoke <package> --id=<old-id>
for package in \
  neo-astra \
  neo-astra-windows-x64 \
  neo-astra-linux-x64 \
  neo-astra-darwin-arm64; do
  npx -y npm@latest trust github "$package" \
    --file release.yml \
    --repository luminousvalentine-cell/Neo-Astra \
    --allow-publish \
    --yes
done
npx -y npm@latest trust list neo-astra --json
```

The `trust list` output must include `"permissions": ["createPackage"]`.
Without `--allow-publish`, the config only permits stage publishing and the
release still fails.

The workflow filename must stay exactly `release.yml`, and the repository must
stay exactly `luminousvalentine-cell/Neo-Astra`. Do not set an environment
unless the release jobs add a matching GitHub environment.

After the first successful OIDC publish, delete the `NPM_TOKEN` secret. It is
already deleted; do not re-add it. If a rollback is needed, create a fresh
short-lived token, use it for that publish only, and revoke it immediately.

## Rulesets

The tag and branch rulesets were applied on 2026-11-07, including the
`pull_request` rule. The JSON below records the active shape.

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

The local pre-push hook allows:

```text
refs/heads/source -> refs/heads/source
```

It rewrites the public staging branch and rejects tests, conformance, docs,
private paths, and build outputs. It no longer permits `source -> master`;
`master` only changes through a pull request.

Mirror release flow:

```sh
cd "C:\Projects\Neo Astra-source"
git push origin source:source
gh pr create --base master --head source --title "Publish sanitized source"
gh pr merge --squash --delete-branch
```

The repository has one maintainer, so the ruleset allows owner self-merge with
zero required approvals. Add required approvals and status checks when a second
maintainer and public CI exist.

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
