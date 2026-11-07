# Contributing to Neo Astra

Neo Astra is owner-reviewed. Pull requests are required; direct pushes to
`master` are blocked. Published tags and release assets are immutable.

## Before you start

- Read the status matrix in [`README.md`](README.md). Do not describe `app` as
  complete, or `web` / `system` as stable, while they are partial or preview.
- Keep changes focused. One behavior change per pull request is easier to
  review and revert.
- Do not add generated output, build products, secrets, or private test files.

## License and sign-off

Neo Astra is licensed under Apache-2.0. By submitting a pull request you agree
that your contribution is licensed under Apache-2.0 and that you have the right
to submit it.

Sign off every commit:

```sh
git commit -s -m "describe the change"
```

The sign-off certifies the Developer Certificate of Origin. No separate CLA is
required.

## Public mirror rules

The public repository is a mirror of the production source. The mirror:

- strips comments from source files, including `.ns` examples;
- excludes the private test suite, conformance corpus, and internal planning
  docs;
- is append-only after a published tag.

Write behavior in the documentation or in tests, not in source comments. A
comment is not part of the public contract.

## Pull request checklist

- [ ] The change is scoped to one problem.
- [ ] Tests cover the success path and each failure mode the change can hit.
- [ ] `cargo fmt --all --check` passes.
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` passes.
- [ ] The relevant test suite passes.
- [ ] Public claims match the real support matrix.
- [ ] No published tag or release archive was rewritten.

Maintainers run the private gate before landing. A pull request without tests
may be accepted for documentation, but not for compiler behavior.

## Commercial users

Apache-2.0 lets companies use, modify, and distribute Neo Astra without a
contribution obligation. We still ask commercial users to contribute fixes,
tests, documentation, and maintenance time. If your company needs a support or
maintenance agreement, open a discussion with the repository owner rather than
assuming the license provides one.

## Code of conduct

Be direct about the code and respectful to the people. Harassment, personal
attacks, and disclosure of private reports are not accepted.
