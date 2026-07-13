Private Translator 0.1.2 for Windows x64
==========================================

FIRST RUN
1. Extract the complete ZIP.
2. While online, double-click Install-Model.exe once. It downloads about 1.13 GB from the pinned official Hy-MT2 revision, supports resume, and verifies size plus SHA-256.
3. Double-click PrivateTranslator-Lite.exe. Use Quality only for its optional private 7B endpoint.

The release ZIP intentionally contains no GGUF model, so application updates stay small. Both executables share the verified model in %LOCALAPPDATA%\PrivateTranslator\models. After setup, ordinary translation works offline.

Normal translation traffic and the encrypted history vault stay on your PC. The only default internet request is the model installation you explicitly start. Quality has no automatic cloud fallback.

Every Quality private-network endpoint, including loopback, requires Bearer authentication. To enable 7B, set PRIVATE_TRANSLATOR_QUALITY_API_KEY before launching the app and start the external llama-server with --api-key using the same value. If unset, only 7B is unavailable and requests to it are blocked before network access. Loopback endpoints at 127.0.0.1 or ::1 may use HTTP. A non-loopback server must additionally use an IP-literal HTTPS URL with a matching IP SAN and a chain to an internal CA trusted by Windows; hostnames and plain HTTP are rejected.

Official public archives require valid Authenticode signatures and RFC 3161 timestamps on the Lite, Quality, and Install-Model executables. The archive contains no executable wrapper scripts. The byte-identical upstream llama-server.exe is verified by pinned size and SHA-256. Archives ending in UNSIGNED-DEVELOPMENT are for local validation only and must not be published. Verify the ZIP using the SHA-256 sidecar. The supply-chain folder contains the CycloneDX SBOM, locked Rust dependency inventory, and upstream crate license texts.
