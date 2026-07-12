Private Translator 0.1.1 for Windows x64
==========================================

FIRST RUN
1. Extract the complete ZIP.
2. While online, double-click Install-Model.cmd once. It downloads about 1.13 GB from the pinned official Hy-MT2 revision, supports resume, and verifies size plus SHA-256.
3. Double-click PrivateTranslator-Lite.exe. Use Quality only for its optional private 7B endpoint.

The release ZIP intentionally contains no GGUF model, so application updates stay small. Both executables share the verified model in %LOCALAPPDATA%\PrivateTranslator\models. After setup, ordinary translation works offline.

Normal translation traffic and the encrypted history vault stay on your PC. The only default internet request is the model installation you explicitly start. Quality has no automatic cloud fallback.

This release is not code-signed. Windows may show an unknown-publisher warning. Verify the ZIP using the SHA-256 sidecar published with the GitHub release.
