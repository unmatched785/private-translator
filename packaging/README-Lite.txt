Private Translator Lite 0.1.2
================================

FIRST RUN
1. Double-click Install-Model.exe while online.
2. It downloads the pinned official Hy-MT2 model (about 1.13 GB), resumes interrupted downloads, and verifies its size and SHA-256.
3. Double-click PrivateTranslator-Lite.exe.
4. Keep the console window open while using the browser interface.

The small application package does not contain the model. By default the verified model is shared by future app updates at:
%LOCALAPPDATA%\PrivateTranslator\models

Advanced commands:
PrivateTranslator-Lite.exe model status
PrivateTranslator-Lite.exe model verify
PrivateTranslator-Lite.exe setup --portable
PrivateTranslator-Lite.exe setup --from C:\path\to\Hy-MT2-1.8B-Q4_K_M.gguf

PRIVACY
- Normal translation traffic stays on 127.0.0.1 and is not sent to an internet translation API.
- Model installation is the only default internet request and starts only when you explicitly run setup.
- Saved content is encrypted with AES-256-GCM using a key protected for the current Windows account.
- Turning off Save history prevents that request from entering the vault.
- Default data location: %LOCALAPPDATA%\PrivateTranslator
- Removing this program folder does not silently delete the vault or shared model.

COMPONENTS
- Hy-MT2 1.8B Q4_K_M for laptop CPUs, installed separately from its pinned official revision
- llama.cpp b9966 Windows x64 CPU runtime, included
- Offline translation after model setup
- Model and runtime SHA-256 verification before launch

NOTICE
Official public archives require valid Authenticode signatures and RFC 3161 timestamps on the Lite, Quality, and Install-Model executables. The byte-identical upstream llama-server.exe is verified by pinned size and SHA-256. A folder from an UNSIGNED-DEVELOPMENT archive is for local validation only and may show an unknown-publisher warning.
