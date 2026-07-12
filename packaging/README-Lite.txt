Private Translator Lite 0.1.0
================================

RUN
1. Double-click PrivateTranslator-Lite.exe.
2. Wait for the translation page to open in your default browser.
3. Keep the console window open while using the browser interface.
4. Close the console window when finished. The managed model process closes with it.

PRIVACY
- Translation requests stay on 127.0.0.1 and are not sent to an internet translation API.
- Saved content is encrypted with AES-256-GCM, using a key protected for the current Windows account.
- Turning off Save history prevents that request from entering the vault.
- Approved translation assets keep encrypted revisions separately from ordinary history.
- Default data location: %LOCALAPPDATA%\PrivateTranslator
- Removing this program folder does not silently delete the vault.

INCLUDED COMPONENTS
- Hy-MT2 1.8B Q4_K_M for laptop CPUs
- llama.cpp b9966 Windows x64 CPU runtime
- No internet connection required after download
- Model and runtime SHA-256 verification before launch
- Loaded model identity verification before translation

NOTICE
This release is not code-signed. Windows may show an unknown-publisher warning.
Model startup can take several seconds.
