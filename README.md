# Private Translator

[한국어](README.ko.md) · [Security](SECURITY.md) · [Contributing](CONTRIBUTING.md)

[![CI](https://github.com/unmatched785/private-translator/actions/workflows/ci.yml/badge.svg)](https://github.com/unmatched785/private-translator/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)

Private Translator is a Windows-first, offline translation app that feels like a web translator without sending your text to a website. The interface opens in your normal browser, while a small local executable runs the model and stores encrypted history on your PC.

> **v0.1 status:** usable and tested on Windows x64, but not code-signed yet. Windows may show an unknown-publisher warning.

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

1. Download `PrivateTranslator-v0.1.0-Windows-x64.zip` from the [latest release](https://github.com/unmatched785/private-translator/releases/latest).
2. Extract the ZIP to a normal folder. Do not run the executable from inside the archive.
3. Double-click `PrivateTranslator-Lite.exe`.
4. Keep its console window open while using the browser interface. Closing it also stops the local model server.

The release archive contains both executables and one shared local model:

| Executable | Intended use | Included model |
| --- | --- | --- |
| `PrivateTranslator-Lite.exe` | Everyday laptops; the recommended default | Hy-MT2 1.8B Q4 |
| `PrivateTranslator-Quality.exe` | The same default model plus an optional private 7B endpoint | Hy-MT2 1.8B Q4 |

Quality never falls back to an internet API. Its optional 7B entry only works after you provide a compatible model server on a permitted private address. The first release does not bundle the much larger 7B model.

Recommended environment: Windows 10 or 11 x64, 8 GB system RAM, and a modern x64 CPU. Translation speed depends heavily on CPU and memory bandwidth. After the archive is downloaded, normal use requires no internet connection.

## Supported languages

The bundled Hy-MT2 model exposes these 38 entries:

Korean, English, Japanese, Simplified Chinese, Traditional Chinese, French, German, Spanish, Portuguese, Italian, Russian, Arabic, Turkish, Thai, Vietnamese, Indonesian, Malay, Filipino, Hindi, Polish, Czech, Dutch, Ukrainian, Hebrew, Persian, Bengali, Tamil, Telugu, Marathi, Gujarati, Urdu, Khmer, Burmese, Tibetan, Kazakh, Mongolian, Uyghur, and Cantonese.

The list comes from the [official Hy-MT2 model card](https://huggingface.co/tencent/Hy-MT2-1.8B-GGUF). Translation quality varies by language pair, domain, and document complexity; the app deliberately does not claim that every pair is equally strong.

## Local history and translation assets

Source text, translated text, language choices, model metadata, latency, and QA warnings are encrypted with AES-256-GCM. On Windows, the random vault key is protected for the current Windows account with DPAPI.

```text
%LOCALAPPDATA%\PrivateTranslator\history.db
%LOCALAPPDATA%\PrivateTranslator\vault.key
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

Clone the repository, then download and verify the pinned model and runtime:

```powershell
.\scripts\bootstrap.ps1
```

Run all checks and build the release executable:

```powershell
.\scripts\check.ps1
$env:CARGO_HOME = Join-Path (Get-Location) '.cache\cargo'
cargo build --locked --release
```

Create portable folders:

```powershell
.\scripts\package.ps1
```

Create the combined release ZIP and SHA-256 sidecar:

```powershell
.\scripts\release.ps1
```

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

## Verification performed for v0.1

- 22 Rust unit and integration-style tests
- `cargo clippy --all-targets -- -D warnings`
- JavaScript syntax checks for the app, localization resource, and mock server
- Real English-to-Korean Hy-MT2 translation
- A 5,938-character office document translated across two chunks without omission
- Model and 51 runtime EXE/DLL hashes checked before launch
- History → approved asset → v2 edit → history deletion with asset preservation
- Plaintext test content absent from the encrypted database file
- Managed `llama-server` terminated when the parent app was force-closed

Machine-specific speed observations are not a performance guarantee.

## Project scope

The next priorities are code signing, encrypted backup and restore, terminology suggestions from explicitly approved local assets, stronger multilingual evaluation, and a supported high-quality model pack. TranslateGemma remains a research candidate because its official model requires a dedicated chat template; it is not exposed as a pretend-compatible option in v0.1.

Please read [CONTRIBUTING.md](CONTRIBUTING.md) before submitting a pull request. Report security issues through the private process in [SECURITY.md](SECURITY.md), not a public issue.

Private Translator is licensed under the [MIT License](LICENSE). Hy-MT2 and `llama.cpp` remain under their respective upstream licenses; see [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md).
