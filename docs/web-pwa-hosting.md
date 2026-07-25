# Web/PWA hosting contract

The web alpha is a static, host-independent application. Git is the source of truth.
The current `chatgpt.site` deployment is one published copy, not a runtime dependency.

## Source and build

- App source: `alpha/`
- Host-independent static output: `dist-alpha/`
- Model and external project URLs: `alpha/config.js`
- Sites adapter output: `alpha/dist/`
- Build commands:

```powershell
npm run alpha:check
npm run alpha:build
```

Upload the contents of `dist-alpha/` to the root of any HTTPS static host. The GGUF files
are deliberately excluded.

The app uses same-origin root-relative URLs for its shell, PWA assets, worker, and runtime.
It does not contain a production app-origin constant. Open Graph image URLs are derived from
the incoming request when a worker adapter is used.

## Model endpoint contract

Every model entry is defined once in `alpha/config.js`:

- immutable upstream revision
- exact filename
- exact byte length
- SHA-256
- remote URL
- local validation URL
- runtime status

The model host must support HTTPS and CORS. HTTP Range support is strongly recommended so a
partial download can resume; the app falls back to a full download when a server ignores a
Range request. A model is never activated until the exact length and SHA-256 pass.

The product target and active model are separate configuration values. The 440.5MB STQ model
remains the target while the 1.13GB Q4 model remains the active stable fallback.

## Required response headers

`alpha/_headers` is a portable header template for hosts such as Cloudflare Pages and Netlify.
Equivalent headers can be configured on another static host:

```text
Cross-Origin-Opener-Policy: same-origin
Cross-Origin-Embedder-Policy: require-corp
Cross-Origin-Resource-Policy: same-origin
Content-Security-Policy: default-src 'self'; script-src 'self' 'wasm-unsafe-eval'; style-src 'self'; img-src 'self' data:; connect-src 'self' https://huggingface.co https://*.huggingface.co https://*.hf.co; worker-src 'self' blob:; manifest-src 'self'; object-src 'none'; base-uri 'none'; frame-ancestors 'none'; form-action 'none'
Permissions-Policy: camera=(), microphone=(), geolocation=(), payment=(), translator=(self)
Referrer-Policy: no-referrer
X-Content-Type-Options: nosniff
```

COOP and COEP enable the multi-threaded WebAssembly path. The current alpha can fall back to
single-threaded CPU preprocessing when an existing host strips those headers, while model
inference remains local and requests WebGPU. A production custom-domain deployment should
serve the full header set and verify `crossOriginIsolated === true`.

When the model host changes, update both `alpha/config.js` and the CSP `connect-src` allowlist.
Do not add a broad wildcard.

## PWA and cache behavior

- `alpha/manifest.webmanifest` defines the installable app at the host root.
- `alpha/sw.js` caches only the small application shell and vendored wllama runtime.
- Model URLs are excluded from the service-worker cache.
- Verified model bytes live in OPFS and metadata lives in local storage.
- Clearing site data removes the browser-owned copy. Exporting or reopening the exact Q4 GGUF
  remains the recovery path.

## Cloudflare custom-domain move

No source change is required to attach a future custom domain when the app is deployed at the
domain root. Configure the domain on the chosen host, preserve HTTPS and the headers above,
then repeat the Chrome compatibility, model download/resume, SHA-256, WebGPU, and restart tests.

Domain purchase and DNS changes are intentionally outside the alpha build.
