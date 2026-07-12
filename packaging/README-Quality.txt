Private Translator Quality 0.1.1
===================================

FIRST RUN
1. Double-click Install-Model.cmd while online to install and verify the shared Hy-MT2 1.8B model (about 1.13 GB).
2. Double-click PrivateTranslator-Quality.exe.
3. Keep the console window open while using the browser interface.

The optional Hy-MT2 7B entry requires a compatible model server on an allowed private address. It never falls back to a cloud translation API. The first Quality release does not include or install a 7B model pack.

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
This release is not code-signed. Windows may show an unknown-publisher warning.
