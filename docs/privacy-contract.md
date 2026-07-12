# Privacy Contract

These are non-negotiable product rules for Private Translator.

1. Never send source text or translated text to an internet service automatically.
2. Never include translation content in URLs, access logs, crash logs, or telemetry.
3. Keep history saving on by default, but let the user disable it per request.
4. Protect saved content and processing metadata with authenticated encryption.
5. Protect the Windows vault key within the current user account boundary.
6. Use browser storage only for non-sensitive preferences.
7. Never make cache deletion or model replacement delete user history.
8. Never make a normal update or uninstall silently delete the vault.
9. Require an explicit user action for deletion, plaintext export, or connection to another model device.
10. Clearly show when a model runs on a private network device instead of this device.
11. Never include a cloud model in the default path or an automatic fallback.
12. Any future use of history for learning or terminology suggestions must remain local and require explicit approval.
13. Never promote ordinary history into an approved translation asset automatically.
14. Preserve previous revisions when an approved asset is edited, and keep history deletion separate from asset deletion.
15. Derive the selectable translation-language list from the active model's declared capability instead of pretending that every model supports every language.
16. Permit internet access for model installation only after an explicit user command, and download only the pinned official artifact described by the embedded trust manifest.
17. Never send translation text, history, approved assets, vault keys, or device telemetry with a model download request.

## Threat boundary

The current product protects database content at rest from casual file inspection and other Windows users. It is not a security product against malware running with the logged-in user's permissions, browser extensions that can inspect local pages, screen capture, keyloggers, operating-system administrators, or memory dumps.

The interface and documentation must describe this limitation without exaggerating the protection.
