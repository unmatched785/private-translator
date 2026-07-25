import { cp, mkdir, readFile, rm, writeFile } from "node:fs/promises";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const repositoryRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const source = resolve(repositoryRoot, "alpha");
const output = resolve(repositoryRoot, "dist-alpha");
const wllama = resolve(repositoryRoot, "node_modules", "@wllama", "wllama", "esm");

await rm(output, { recursive: true, force: true });
await mkdir(resolve(output, "vendor", "wllama", "wasm"), { recursive: true });
await cp(source, output, { recursive: true });
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
