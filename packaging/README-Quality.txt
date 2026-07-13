Private Translator Quality 0.1.2
===================================

FIRST RUN
1. Double-click Install-Model.exe while online to install and verify the shared Hy-MT2 1.8B model (about 1.13 GB).
2. Double-click PrivateTranslator-Quality.exe.
3. Keep the console window open while using the browser interface.

The optional Hy-MT2 7B entry requires a compatible model server on an allowed private address. Every private-network endpoint, including loopback, requires Bearer authentication. To enable 7B, set PRIVATE_TRANSLATOR_QUALITY_API_KEY before launching the app and start the external server with `llama-server --api-key` using the same value. Without a non-empty value, only 7B is unavailable and the managed 1.8B model keeps working; direct 7B requests are rejected before network access. Loopback servers at 127.0.0.1 or ::1 may use HTTP. A non-loopback server must additionally use an IP-literal HTTPS URL; hostnames and plain HTTP are rejected, and its certificate needs a matching IP SAN and a chain to an internal CA trusted by Windows. It never falls back to a cloud translation API. The first Quality release does not include or install a 7B model pack.

MODEL ENDPOINTS
- Hy-MT2 1.8B: http://127.0.0.1:8080/v1 (managed automatically)
- Hy-MT2 7B: http://127.0.0.1:8081/v1 (optional private endpoint)

SHARED LOCAL MODEL
%LOCALAPPDATA%\PrivateTranslator\models

Useful commands:
PrivateTranslator-Quality.exe model status
PrivateTranslator-Quality.exe model verify
PrivateTranslator-Quality.exe setup --portable

PRIVACY
- Normal translation has no external internet API or automatic cloud fallback.
- Model installation is user-initiated and downloads only the pinned official file.
- History and approved translation asset revisions stay in this Windows PC's encrypted vault.

NOTICE
Official public archives require valid Authenticode signatures and RFC 3161 timestamps on the Lite, Quality, and Install-Model executables. The byte-identical upstream llama-server.exe is verified by pinned size and SHA-256. A folder from an UNSIGNED-DEVELOPMENT archive is for local validation only and may show an unknown-publisher warning.
