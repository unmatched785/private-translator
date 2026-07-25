import { createReadStream } from "node:fs";
import { stat } from "node:fs/promises";
import { createServer } from "node:http";
import { extname, join, normalize, resolve, sep } from "node:path";
import { fileURLToPath } from "node:url";

const root = resolve(fileURLToPath(new URL("./", import.meta.url)));
const repositoryRoot = resolve(root, "..");
const vendorRoot = resolve(repositoryRoot, "node_modules", "@wllama", "wllama", "esm");
const modelPaths = new Map([
  [
    "/model/Hy-MT2-1.8B-Q4_K_M.gguf",
    resolve(repositoryRoot, "models", "Hy-MT2-1.8B-Q4_K_M.gguf"),
  ],
  [
    "/model/Hy-MT2-1.8B-1.25Bit.gguf",
    resolve(repositoryRoot, "models", "Hy-MT2-1.8B-1.25Bit.gguf"),
  ],
]);
const port = Number(process.env.VALIDATION_PORT ?? 8191);
const contentTypes = {
  ".html": "text/html; charset=utf-8",
  ".js": "text/javascript; charset=utf-8",
  ".json": "application/json; charset=utf-8",
  ".wasm": "application/wasm",
  ".gguf": "application/octet-stream",
};

function isWithin(candidate, allowedRoot) {
  return candidate === allowedRoot || candidate.startsWith(`${allowedRoot}${sep}`);
}

function resolveRequestPath(requestPath) {
  if (modelPaths.has(requestPath)) return modelPaths.get(requestPath);
  if (requestPath.startsWith("/vendor/wllama/")) {
    const candidate = normalize(
      join(vendorRoot, decodeURIComponent(requestPath.slice("/vendor/wllama/".length))),
    );
    return isWithin(candidate, vendorRoot) ? candidate : null;
  }

  const relativePath =
    requestPath === "/" ? "wllama-hymt2.html" : decodeURIComponent(requestPath.slice(1));
  const candidate = normalize(join(root, relativePath));
  return isWithin(candidate, root) ? candidate : null;
}

function parseRange(rangeHeader, size) {
  if (!rangeHeader) return null;
  const match = /^bytes=(\d*)-(\d*)$/.exec(rangeHeader.trim());
  if (!match) return false;

  let start;
  let end;
  if (match[1] === "") {
    const suffixLength = Number(match[2]);
    if (!Number.isSafeInteger(suffixLength) || suffixLength <= 0) return false;
    start = Math.max(size - suffixLength, 0);
    end = size - 1;
  } else {
    start = Number(match[1]);
    end = match[2] === "" ? size - 1 : Number(match[2]);
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
  const requestPath = new URL(request.url ?? "/", `http://${request.headers.host}`).pathname;
  process.stdout.write(
    `${new Date().toISOString()} ${request.method ?? "GET"} ${requestPath}${request.headers.range ? ` ${request.headers.range}` : ""}\n`,
  );
  const filePath = resolveRequestPath(requestPath);
  if (!filePath) {
    response.writeHead(403).end("Forbidden");
    return;
  }

  try {
    const metadata = await stat(filePath);
    if (!metadata.isFile()) throw new Error("Not a file");
    const range = parseRange(request.headers.range, metadata.size);
    if (range === false) {
      response.writeHead(416, { "content-range": `bytes */${metadata.size}` }).end();
      return;
    }

    const status = range ? 206 : 200;
    const start = range?.start ?? 0;
    const end = range?.end ?? metadata.size - 1;
    const headers = {
      "content-type": contentTypes[extname(filePath)] ?? "application/octet-stream",
      "content-length": end - start + 1,
      "cache-control": "no-store",
      "accept-ranges": "bytes",
      "cross-origin-opener-policy": "same-origin",
      "cross-origin-embedder-policy": "require-corp",
      "cross-origin-resource-policy": "same-origin",
      "permissions-policy": "translator=(self)",
    };
    if (range) headers["content-range"] = `bytes ${start}-${end}/${metadata.size}`;
    response.writeHead(status, headers);
    if (request.method === "HEAD") {
      response.end();
      return;
    }
    createReadStream(filePath, { start, end }).pipe(response);
  } catch {
    response.writeHead(404).end("Not found");
  }
}).listen(port, "127.0.0.1", () => {
  process.stdout.write(`Validation server: http://127.0.0.1:${port}\n`);
});
