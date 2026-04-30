# Security Policy

## Reporting a Vulnerability

Email **o.krylow@isp-insoft.de** with details. Do not open a public issue for security reports.

Please include:

- Affected version (commit hash or release tag).
- Reproduction steps or proof-of-concept.
- Impact assessment.

You will receive an acknowledgement within 5 business days. Coordinated disclosure is preferred — we'll agree on a public-disclosure date once a fix is ready.

## Supported Versions

Only the latest release on `trunk` receives security fixes. There is no LTS branch.

## Scope

In scope: the `kree.exe` binary built from this repository, and the official release artifacts published via GitHub Releases.

Out of scope: third-party forks, modified builds, and vulnerabilities in upstream dependencies (report those to the upstream project — `cargo audit` flags the latter automatically in CI).
