# Private Translator

[한국어](README.ko.md) · [Security](SECURITY.md) · [Contributing](CONTRIBUTING.md)

[![CI](https://github.com/unmatched785/private-translator/actions/workflows/ci.yml/badge.svg)](https://github.com/unmatched785/private-translator/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)

Private Translator is a Windows-first, offline translation app that feels like a web translator without sending your text to a website. The interface opens in your normal browser, while a small local executable runs the model and stores encrypted history on your PC.

> **Release trust:** v0.1.1 was published before signing became mandatory. The current release tool refuses to create a public archive unless the Lite, Quality, and Install-Model executables all have valid Authenticode signatures and RFC 3161 timestamps. Local unsigned builds are clearly named `UNSIGNED-DEVELOPMENT` and are not release artifacts.

## Web/PWA alpha

The `codex/web-pwa-alpha` branch contains the smallest Chrome-first public alpha. It is a static website/PWA, not an extension, and supports only direct Korean ↔ English translation.

**Public alpha:** [Open Private Translator Alpha](https://private-translator-alpha.wise-goby-3000.chatgpt.site) in current desktop Chrome.

- The exact 1,133,080,448-byte Hy-MT2 Q4 model is downloaded separately from a pinned upstream revision.
- A partial download remains in browser storage and can resume. The model is activated only after its exact size and SHA-256 pass.
- Users who keep the GGUF outside the browser can open that file again instead of downloading it again.
- Source text and translation results stay inside the Chrome tab. Model download requests can still expose normal network metadata such as IP address, time, and requested model revision to the file host.
- Clearing site data can remove the browser copy even when persistent storage was granted. Keeping the GGUF as a normal file is the recovery path.
- The alpha intentionally has no account, sync, history, glossary, page translation, browser extension, analytics, or cloud fallback.

The only early product questions are whether a person finishes the 1.13GB download, translates their own sentence, and returns to use it again. GitHub release `download_count` and repository traffic are interest signals only; they do **not** prove model installation, translation, or repeat use.

Local development:

```powershell
npm install
npm run alpha:check
npm run alpha:build
npm run alpha:serve
```

Open `http://127.0.0.1:8192`. Add `?source=local` only for local validation against `models/Hy-MT2-1.8B-Q4_K_M.gguf`.

## What it does

- Runs Tencent Hy-MT2 1.8B Q4 locally through `llama.cpp`
- Supports 38 language entries from the official Hy-MT2 language table
- Translates automatically when text is pasted, or with `Ctrl+Enter`
- Splits long documents by token budget and retries truncated chunks
- Keeps searchable translation history in an encrypted local vault
- Lets you approve good translations as reusable assets with immutable revisions
- Checks whether numbers and URLs were lost or changed
- Verifies the model, runtime files, and loaded model identity before use
- Binds only to loopback and has no cloud fallback, telemetry, remote fonts, or analytics
- Offers an English interface by default with a Korean interface switch

## Download and run

1. Download `PrivateTranslator-v0.1.2-Windows-x64.zip` from the [latest release](https://github.com/unmatched785/private-translator/releases/latest).
2. Extract the ZIP to a normal folder. Do not run the executable from inside the archive.
3. While online, double-click `Install-Model.exe` once. It downloads about 1.13 GB from a pinned official Hy-MT2 revision, resumes interrupted downloads, and verifies the exact size and SHA-256.
4. Double-click `PrivateTranslator-Lite.exe`.
5. Keep its console window open while using the browser interface. Closing it also stops the local model server.

The small release archive contains two translation launchers, the signed `Install-Model.exe`, and the pinned `llama.cpp` runtime, but no GGUF model. Both profiles use the one verified model installed at `%LOCALAPPDATA%\PrivateTranslator\models`:

| Executable | Intended use | Default model |
| --- | --- | --- |
| `PrivateTranslator-Lite.exe` | Everyday laptops; the recommended default | Shared Hy-MT2 1.8B Q4 |
| `PrivateTranslator-Quality.exe` | The same default model plus an optional private 7B endpoint | Shared Hy-MT2 1.8B Q4 |

Quality never falls back to an internet API. Its optional 7B entry only works after you provide a compatible model server on a permitted private address. Every private-network endpoint, including loopback, must use Bearer authentication. To enable the 7B entry, set the dedicated `PRIVATE_TRANSLATOR_QUALITY_API_KEY` environment variable before launching the app and pass the same secret to the external server with `llama-server --api-key`. Without a non-empty secret, only the 7B entry is marked unavailable; the managed 1.8B model keeps working, and direct 7B API requests are rejected before any network connection. Loopback endpoints at `127.0.0.1` or `::1` may use HTTP. Non-loopback private servers must use an IP-literal HTTPS URL; hostnames and plain HTTP are rejected, and the certificate must contain that address as an IP Subject Alternative Name and chain to an internal CA trusted by Windows. v0.1.2 does not install the much larger 7B model.

```powershell
$env:PRIVATE_TRANSLATOR_QUALITY_API_KEY = 'one-long-random-secret'
.\runtime\llama.cpp\llama-server.exe --model "C:\models\Hy-MT2-7B-Q4_K_M.gguf" --alias hy-mt2-7b --host 127.0.0.1 --port 8081 --ctx-size 4096 --api-key $env:PRIVATE_TRANSLATOR_QUALITY_API_KEY
```

Launch `PrivateTranslator-Quality.exe` from a second PowerShell window where `PRIVATE_TRANSLATOR_QUALITY_API_KEY` has been set to the same value. If the optional server is not needed, launch Quality normally and use its managed 1.8B model.

Recommended environment: Windows 10 or 11 x64, 8 GB system RAM, and a modern x64 CPU. Translation speed depends heavily on CPU and memory bandwidth. After model setup, normal use requires no internet connection. The original [v0.1.0 full offline bundle](https://github.com/unmatched785/private-translator/releases/tag/v0.1.0) remains available for air-gapped transfer.

Useful model commands:

```powershell
.\PrivateTranslator-Lite.exe model status
.\PrivateTranslator-Lite.exe model verify
.\PrivateTranslator-Lite.exe setup --portable
.\PrivateTranslator-Lite.exe setup --from C:\path\to\Hy-MT2-1.8B-Q4_K_M.gguf
```

## Supported languages

The default Hy-MT2 model exposes these 38 entries:

Korean, English, Japanese, Simplified Chinese, Traditional Chinese, French, German, Spanish, Portuguese, Italian, Russian, Arabic, Turkish, Thai, Vietnamese, Indonesian, Malay, Filipino, Hindi, Polish, Czech, Dutch, Ukrainian, Hebrew, Persian, Bengali, Tamil, Telugu, Marathi, Gujarati, Urdu, Khmer, Burmese, Tibetan, Kazakh, Mongolian, Uyghur, and Cantonese.

The list comes from the [official Hy-MT2 model card](https://huggingface.co/tencent/Hy-MT2-1.8B-GGUF). Translation quality varies by language pair, domain, and document complexity; the app deliberately does not claim that every pair is equally strong.

## Local history and translation assets

Source text, translated text, language choices, model metadata, latency, and QA warnings are encrypted with AES-256-GCM. On Windows, the random vault key is protected for the current Windows account with DPAPI.

```text
%LOCALAPPDATA%\PrivateTranslator\history.db
%LOCALAPPDATA%\PrivateTranslator\vault.key
%LOCALAPPDATA%\PrivateTranslator\models\Hy-MT2-1.8B-Q4_K_M.gguf
```

The database keeps only structural metadata such as random IDs and timestamps outside the ciphertext. Turning off **Save history** skips storage for that request. Deleting the app folder does not silently delete the vault.

History and approved translation assets are intentionally separate. Deleting a history event does not delete its approved asset. Editing an approved asset creates v2, v3, and later encrypted revisions instead of overwriting prior work.

This protects data at rest from casual file inspection and other Windows users. It is not a defense against malware running as your logged-in account, keyloggers, screen capture, administrators, or memory inspection. See the [privacy contract](docs/privacy-contract.md) for the exact boundary.

## Build from source

Prerequisites:

- Windows 10/11 x64
- Rust stable
- PowerShell 7 or Windows PowerShell 5.1
- Node.js only for JavaScript syntax checks and the UI-only mock server

Clone the repository, then download and verify the pinned development model and runtime:

```powershell
.\scripts\bootstrap.ps1
```

Run all checks and build the release executable:

```powershell
.\scripts\check.ps1
$env:CARGO_HOME = Join-Path (Get-Location) '.cache\cargo'
cargo build --locked --release
```

Create thin portable folders. The resulting packages intentionally exclude every `.gguf` file and copy only the verified `llama-server` runtime closure. When the pinned development model is present, packaging also performs a real Windows model-loading smoke test:

```powershell
.\scripts\package.ps1
```

Create the compressed thin public release ZIP and SHA-256 sidecar. A public release requires a code-signing certificate, SHA-256 Authenticode, and an HTTPS RFC 3161 timestamp service for the Lite, Quality, and Install-Model executables. `Install-Model.exe` is built from the same Rust binary and enters the explicit `setup` path only when launched without arguments under that exact filename. The release gate rejects executable scripts such as CMD, BAT, PowerShell, and VBS files. The upstream `llama-server.exe` remains byte-identical to the pinned llama.cpp release and is verified by size and SHA-256 instead of being re-signed. The archive also includes a CycloneDX SBOM, a Cargo-metadata dependency inventory, and the actual upstream license files for every locked Windows Rust dependency; any missing license fails the release:

```powershell
$env:PRIVATE_TRANSLATOR_SIGNING_CERT_THUMBPRINT = '40-HEX-CERTIFICATE-THUMBPRINT'
$env:PRIVATE_TRANSLATOR_RFC3161_TIMESTAMP_URL = 'https://your-rfc3161-timestamp-service'
# Optional when signtool.exe is not on PATH:
$env:PRIVATE_TRANSLATOR_SIGNTOOL = 'C:\Program Files (x86)\Windows Kits\10\bin\...\x64\signtool.exe'
.\scripts\release.ps1
```

For local packaging validation only, create an unmistakably named unsigned artifact:

```powershell
.\scripts\release.ps1 -UnsignedDevelopment
```

Never publish an `UNSIGNED-DEVELOPMENT` archive.

For UI work without downloading the model:

```powershell
node .\scripts\mock-server.mjs
```

Then open `http://127.0.0.1:8173`.

## Architecture

```text
Default browser
    ↕ 127.0.0.1 only
Private Translator executable
    ├─ encrypted SQLite vault + Windows DPAPI key protection
    └─ managed llama.cpp process
           └─ pinned Hy-MT2 GGUF model
```

The application verifies SHA-256 hashes before launching the model server, verifies the model ID returned by that server, serializes work per model, and gives the newest request from each browser tab priority at safe cancellation points.

## Verification performed for v0.1.2

- 48 Rust unit and integration-style tests, including an actual HTTP Range resume, Windows file flush, and final checksum verification
- `cargo clippy --all-targets -- -D warnings`
- JavaScript syntax checks for the app, localization resource, and mock server
- Real English-to-Korean Hy-MT2 translation
- A 5,938-character office document translated across two chunks without omission
- Model and the minimized 23-file `llama-server` runtime closure checked before launch
- History → approved asset → v2 edit → history deletion with asset preservation
- Plaintext test content absent from the encrypted database file
- Managed `llama-server` terminated when the parent app was force-closed
- Thin release inspection proving that no GGUF model or unrelated llama.cpp tool is bundled
- Full model-loading smoke test against the minimized Windows runtime
- Cargo-metadata license inventory, copied upstream license texts, and CycloneDX 1.5 SBOM
- Shared and portable model installation paths with pinned size and SHA-256 verification

Machine-specific speed observations are not a performance guarantee.

## Project scope

The next priorities are encrypted backup and restore, terminology suggestions from explicitly approved local assets, stronger multilingual evaluation, and a supported high-quality model pack. TranslateGemma remains a research candidate because its official model requires a dedicated chat template; it is not exposed as a pretend-compatible option in v0.1.

Please read [CONTRIBUTING.md](CONTRIBUTING.md) before submitting a pull request. Report security issues through the private process in [SECURITY.md](SECURITY.md), not a public issue.

Private Translator is licensed under the [MIT License](LICENSE). Hy-MT2 and `llama.cpp` remain under their respective upstream licenses; see [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md).
