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
- reject public internet model endpoints and hostnames; every `private_network` endpoint, including `127.0.0.1` and `::1`, must declare an `api_key_env` whose environment value is sent as its Bearer secret. The built-in Quality endpoint uses `PRIVATE_TRANSLATOR_QUALITY_API_KEY`, and the external `llama-server` must receive the same value through `--api-key`. A missing or empty secret marks only that optional model unavailable and rejects direct translation before any network connection. Loopback endpoints may use HTTP; a non-loopback endpoint must additionally be an IP-literal HTTPS URL with a matching IP SAN in its certificate and a chain to an internal CA trusted by Windows;
- encrypt saved content at rest;
- protect the Windows vault key with current-user DPAPI;
- avoid putting translation content in URLs, logs, telemetry, and analytics;
- verify pinned runtime and model artifacts before execution.
- require valid SHA-256 Authenticode signatures and RFC 3161 timestamps on the Lite, Quality, and Install-Model executables in every public release, and reject packaged executable scripts; the byte-identical upstream `llama-server.exe` is instead pinned and verified by size and SHA-256;
- publish a CycloneDX SBOM, locked Rust dependency inventory, and the corresponding upstream license texts in the release archive.

It does not protect against malware running as the logged-in user, keyloggers, screen capture, administrators, browser extensions that can read local pages, or memory inspection. Source builds and archives explicitly ending in `UNSIGNED-DEVELOPMENT` do not provide publisher identity and must not be published as releases. The release script fails closed when the signing certificate, RFC 3161 timestamp service, or signature verification is unavailable.

Please read [docs/privacy-contract.md](docs/privacy-contract.md) for the complete product boundary.
