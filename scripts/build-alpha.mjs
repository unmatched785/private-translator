import { cp, mkdir, readFile, rm, writeFile } from "node:fs/promises";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const repositoryRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const source = resolve(repositoryRoot, "alpha");
const output = resolve(repositoryRoot, "dist-alpha");
const wllama = resolve(repositoryRoot, "node_modules", "@wllama", "wllama", "esm");
const assets = [
  "_headers",
  "app.js",
  "config.js",
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
await mkdir(resolve(output, "vendor", "wllama", "wasm"), { recursive: true });
for (const asset of assets) {
  await cp(resolve(source, asset), resolve(output, asset));
}
await cp(resolve(wllama, "index.js"), resolve(output, "vendor", "wllama", "index.js"));
await cp(
  resolve(wllama, "wasm", "wllama.wasm"),
  resolve(output, "vendor", "wllama", "wasm", "wllama.wasm"),
);

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

process.stdout.write(`Built web alpha at ${output}\n`);
