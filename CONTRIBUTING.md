# Contributing

Thanks for helping improve Private Translator. Privacy and predictable offline behavior are product requirements, not optional features.

## Before opening a change

- Search existing issues and pull requests.
- Use an issue for a substantial feature or a change to the privacy boundary.
- Use the private process in [SECURITY.md](SECURITY.md) for vulnerabilities.
- Keep pull requests focused. Unrelated formatting or dependency changes make security review harder.

## Local setup

The packaged application currently targets Windows x64.

```powershell
.\scripts\bootstrap.ps1
.\scripts\check.ps1
```

`bootstrap.ps1` downloads pinned third-party binaries and a 1.13 GB model. UI-only work can avoid that download:

```powershell
node .\scripts\mock-server.mjs
```

Open `http://127.0.0.1:8173`.

## Pull request checklist

- Explain the user-visible outcome and privacy impact.
- Add or update tests for behavioral changes.
- Run `scripts\check.ps1`.
- Keep all translation content out of URLs, logs, telemetry, and crash reports.
- Do not add an external model endpoint, cloud fallback, remote font, or analytics service by default.
- Preserve existing vault data and migration paths.
- Update English and Korean UI resources together when adding interface text.
- Update the README or architecture notes when behavior or packaging changes.

## Localization

The interface uses `web/i18n.js` with English as the fallback. Add a stable message key, update every supported interface locale, and keep dynamic error codes language-neutral. Model-supported translation languages are supplied by the local API; do not hard-code a larger list than a model actually supports.

## Model and runtime updates

Never update a download URL without updating and independently checking its size, SHA-256, upstream license, and source reference. Changes to `packaging/trusted-artifacts.json` need especially careful review.

By contributing, you agree that your contribution is licensed under the repository's MIT License.
