import { cp, mkdir, readFile, rm, writeFile } from "node:fs/promises";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const repositoryRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const source = resolve(repositoryRoot, "alpha");
const output = resolve(source, "dist");
const client = resolve(output, "client");
const server = resolve(output, "server");
const wllama = resolve(repositoryRoot, "node_modules", "@wllama", "wllama", "esm");
const assets = [
  "_headers",
  "app.js",
  "icon-192.png",
  "icon-512.png",
  "index.html",
  "manifest.webmanifest",
  "og.png",
  "sha256.js",
  "styles.css",
  "sw.js",
];

await rm(output, { recursive: true, force: true });
await mkdir(resolve(client, "vendor", "wllama", "wasm"), { recursive: true });
await mkdir(server, { recursive: true });

for (const asset of assets) {
  await cp(resolve(source, asset), resolve(client, asset));
}
await cp(resolve(wllama, "index.js"), resolve(client, "vendor", "wllama", "index.js"));
await cp(
  resolve(wllama, "wasm", "wllama.wasm"),
  resolve(client, "vendor", "wllama", "wasm", "wllama.wasm"),
);

const workerSource = `
const SECURITY_HEADERS = {
  "Cross-Origin-Opener-Policy": "same-origin",
  "Cross-Origin-Embedder-Policy": "require-corp",
  "Cross-Origin-Resource-Policy": "same-origin",
  "Content-Security-Policy": "default-src 'self'; script-src 'self' 'wasm-unsafe-eval'; style-src 'self'; img-src 'self' data:; connect-src 'self' https://huggingface.co https://*.huggingface.co https://*.hf.co; worker-src 'self' blob:; manifest-src 'self'; object-src 'none'; base-uri 'none'; frame-ancestors 'none'; form-action 'none'",
  "Permissions-Policy": "camera=(), microphone=(), geolocation=(), payment=(), translator=(self)",
  "Referrer-Policy": "no-referrer",
  "X-Content-Type-Options": "nosniff",
};

export default {
  async fetch(request, env) {
    const url = new URL(request.url);
    const asset = await env.ASSETS.fetch(request);
    const headers = new Headers(asset.headers);
    for (const [name, value] of Object.entries(SECURITY_HEADERS)) headers.set(name, value);

    if (
      request.method === "GET" &&
      asset.ok &&
      (url.pathname === "/" || url.pathname === "/index.html")
    ) {
      const body = (await asset.text()).replaceAll('content="/og.png"', \`content="\${url.origin}/og.png"\`);
      headers.delete("content-length");
      return new Response(body, { status: asset.status, statusText: asset.statusText, headers });
    }

    return new Response(asset.body, {
      status: asset.status,
      statusText: asset.statusText,
      headers,
    });
  },
};
`.trimStart();

await writeFile(resolve(server, "index.js"), workerSource);

const packageJson = JSON.parse(await readFile(resolve(repositoryRoot, "package.json"), "utf8"));
await writeFile(
  resolve(output, "build.json"),
  `${JSON.stringify(
    {
      name: packageJson.name,
      version: packageJson.version,
      builtAt: new Date().toISOString(),
      modelIncluded: false,
    },
    null,
    2,
  )}\n`,
);

process.stdout.write(`Built Sites alpha at ${output}\n`);
