Private Translator Quality 0.1.0
===================================

RUN
1. Double-click PrivateTranslator-Quality.exe.
2. Wait for the translation page to open in your default browser.
3. The bundled Hy-MT2 1.8B model works immediately.
4. The optional Hy-MT2 7B entry requires a compatible private model server.
5. Close the console window when finished. Any model process started by this app closes with it.

MODEL ENDPOINTS
- Hy-MT2 1.8B: http://127.0.0.1:8080/v1 (managed automatically)
- Hy-MT2 7B: http://127.0.0.1:8081/v1 (optional private endpoint)

PRIVACY
- There is no external internet API or automatic cloud fallback.
- Custom private servers are restricted to loopback, private IP, .local, single-label, or Tailscale CGNAT addresses.
- History and approved translation asset revisions stay in this Windows PC's encrypted vault.
- Default data location: %LOCALAPPDATA%\PrivateTranslator

NOTICE
The first release does not include the much larger 7B model pack.
This release is not code-signed. Windows may show an unknown-publisher warning.
