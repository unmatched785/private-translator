import { readFile, stat } from "node:fs/promises";
import { resolve } from "node:path";
import { Sha256 } from "../alpha/sha256.js";

const root = resolve(import.meta.dirname, "..");
const required = [
  "alpha/index.html",
  "alpha/app.js",
  "alpha/icon-192.png",
  "alpha/icon-512.png",
  "alpha/og.png",
  "alpha/sha256.js",
  "alpha/styles.css",
  "alpha/manifest.webmanifest",
  "alpha/sw.js",
  "alpha/_headers",
];

for (const relative of required) {
  const metadata = await stat(resolve(root, relative));
  if (!metadata.isFile() || metadata.size === 0) throw new Error(`Missing alpha asset: ${relative}`);
}

const vectors = [
  ["", "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"],
  ["abc", "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"],
  [
    "The quick brown fox jumps over the lazy dog",
    "d7a8fbb307d7809469ca9abcb0082e4f8d5651e46d3cdb762d02d0bf37c9e592",
  ],
];

for (const [input, expected] of vectors) {
  const hasher = new Sha256();
  const encoded = new TextEncoder().encode(input);
  for (let offset = 0; offset < encoded.length; offset += 3) {
    hasher.update(encoded.subarray(offset, offset + 3));
  }
  const actual = hasher.hex();
  if (actual !== expected) throw new Error(`SHA-256 vector failed: ${input || "(empty)"}`);
}

const manifest = JSON.parse(await readFile(resolve(root, "alpha/manifest.webmanifest"), "utf8"));
if (manifest.start_url !== "/") throw new Error("Unexpected PWA start_url");
process.stdout.write("Alpha static and SHA-256 checks passed.\n");
