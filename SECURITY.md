# Security Policy

## Supported versions

Security fixes are applied to the latest release and the default branch. Pre-release model packs and custom configurations are best-effort only.

## Reporting a vulnerability

Do not open a public issue for a vulnerability or include private translation content in a report.

Use GitHub's private vulnerability reporting form:

<https://github.com/unmatched785/private-translator/security/advisories/new>

Include the affected version, Windows version, reproduction steps using synthetic text, impact, and any suggested mitigation. You should receive an acknowledgement within seven days. A fix timeline depends on severity and reproducibility.

## Security boundary

Private Translator aims to:

- keep translation traffic on loopback or an explicitly configured private network;
- reject public internet model endpoints;
- encrypt saved content at rest;
- protect the Windows vault key with current-user DPAPI;
- avoid putting translation content in URLs, logs, telemetry, and analytics;
- verify pinned runtime and model artifacts before execution.

It does not protect against malware running as the logged-in user, keyloggers, screen capture, administrators, browser extensions that can read local pages, or memory inspection. An unsigned release also cannot provide publisher identity through Windows SmartScreen.

Please read [docs/privacy-contract.md](docs/privacy-contract.md) for the complete product boundary.
