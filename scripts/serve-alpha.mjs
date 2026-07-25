import { createReadStream } from "node:fs";
import { stat } from "node:fs/promises";
import { createServer } from "node:http";
import { extname, join, normalize, resolve, sep } from "node:path";
import { fileURLToPath } from "node:url";

const repositoryRoot = resolve(fileURLToPath(new URL("../", import.meta.url)));
const root = resolve(repositoryRoot, "dist-alpha");
const localModelPaths = new Map([
  [
    "/model/Hy-MT2-1.8B-Q4_K_M.gguf",
    resolve(repositoryRoot, "models", "Hy-MT2-1.8B-Q4_K_M.gguf"),
  ],
  [
    "/model/Hy-MT2-1.8B-1.25Bit.gguf",
    resolve(repositoryRoot, "models", "Hy-MT2-1.8B-1.25Bit.gguf"),
  ],
]);
const port = Number(process.env.ALPHA_PORT ?? 8192);
const contentTypes = {
  ".css": "text/css; charset=utf-8",
  ".html": "text/html; charset=utf-8",
  ".js": "text/javascript; charset=utf-8",
  ".json": "application/json; charset=utf-8",
  ".png": "image/png",
  ".wasm": "application/wasm",
  ".webmanifest": "application/manifest+json; charset=utf-8",
  ".gguf": "application/octet-stream",
};

function isWithin(candidate, allowedRoot) {
  return candidate === allowedRoot || candidate.startsWith(`${allowedRoot}${sep}`);
}

function parseRange(header, size) {
  if (!header) return null;
  const match = /^bytes=(\d*)-(\d*)$/.exec(header.trim());
  if (!match) return false;

  let start;
  let end;
  if (!match[1]) {
    const suffix = Number(match[2]);
    if (!Number.isSafeInteger(suffix) || suffix <= 0) return false;
    start = Math.max(0, size - suffix);
    end = size - 1;
  } else {
    start = Number(match[1]);
    end = match[2] ? Number(match[2]) : size - 1;
  }
  if (
    !Number.isSafeInteger(start) ||
    !Number.isSafeInteger(end) ||
    start < 0 ||
    start >= size ||
    end < start
  ) {
    return false;
  }
  return { start, end: Math.min(end, size - 1) };
}

createServer(async (request, response) => {
  const url = new URL(request.url ?? "/", `http://${request.headers.host}`);
  let filePath;
  if (localModelPaths.has(url.pathname)) {
    filePath = localModelPaths.get(url.pathname);
  } else {
    const relative = decodeURIComponent(url.pathname === "/" ? "index.html" : url.pathname.slice(1));
    const candidate = normalize(join(root, relative));
    if (!isWithin(candidate, root)) {
      response.writeHead(403).end("Forbidden");
      return;
    }
    filePath = candidate;
  }

  try {
    const metadata = await stat(filePath);
    if (!metadata.isFile()) throw new Error("Not a file");
    const range = parseRange(request.headers.range, metadata.size);
    if (range === false) {
      response.writeHead(416, { "content-range": `bytes */${metadata.size}` }).end();
      return;
    }

    const start = range?.start ?? 0;
    const end = range?.end ?? metadata.size - 1;
    const headers = {
      "accept-ranges": "bytes",
      "cache-control": extname(filePath) === ".gguf" ? "no-store" : "no-cache",
      "content-length": end - start + 1,
      "content-type": contentTypes[extname(filePath)] ?? "application/octet-stream",
      "cross-origin-embedder-policy": "require-corp",
      "cross-origin-opener-policy": "same-origin",
      "cross-origin-resource-policy": "same-origin",
      "content-security-policy":
        "default-src 'self'; script-src 'self' 'wasm-unsafe-eval'; style-src 'self'; img-src 'self'; connect-src 'self' https://huggingface.co https://*.hf.co; worker-src 'self' blob:; object-src 'none'; base-uri 'self'; frame-ancestors 'none'; form-action 'self'",
      "permissions-policy": "translator=(self)",
      "referrer-policy": "no-referrer",
      "x-content-type-options": "nosniff",
    };
    if (range) headers["content-range"] = `bytes ${start}-${end}/${metadata.size}`;
    response.writeHead(range ? 206 : 200, headers);
    if (request.method === "HEAD") return response.end();
    createReadStream(filePath, { start, end }).pipe(response);
  } catch {
    response.writeHead(404).end("Not found");
  }
}).listen(port, "127.0.0.1", () => {
  process.stdout.write(`Private Translator alpha: http://127.0.0.1:${port}\n`);
});
