# Security policy

## Supported versions

Security fixes are provided for the current `1.0.x` release line. Older
releases may be asked to upgrade before a fix is backported.

## Reporting a vulnerability

Do **not** open a public issue for an unfixed vulnerability.

Use the repository's private security advisory flow:

1. Open the **Security** tab on the public repository.
2. Choose **Report a vulnerability**.
3. Include the affected version, host, a minimal reproduction, and the impact.

If the advisory flow is unavailable, contact the repository owner through the
GitHub profile linked from the repository before disclosing anything publicly.

## What to expect

- Acknowledgement of the report.
- Confirmation or rejection with a reason.
- A coordinated fix and release for accepted reports.
- Credit in the release notes unless you ask to remain anonymous.

## Scope notes

- The compiler runs on untrusted source. A panic, hang, or parser crash on
  malformed input is a bug worth reporting.
- Release archives carry SHA-256 sidecars. A checksum mismatch means the
  download is corrupt; it does not by itself prove malicious replacement.
- Reports about a missing feature are not security issues unless the feature
  is advertised as present and silently falls back to an unsafe behavior.
