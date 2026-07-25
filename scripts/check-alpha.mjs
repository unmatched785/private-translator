import { readFile, stat } from "node:fs/promises";
import { resolve } from "node:path";
import { APP_CONFIG } from "../alpha/config.js";
import { Sha256 } from "../alpha/sha256.js";

const root = resolve(import.meta.dirname, "..");
const required = [
  "alpha/index.html",
  "alpha/app.js",
  "alpha/config.js",
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

const indexHtml = await readFile(resolve(root, "alpha/index.html"), "utf8");
for (const id of [
  "sourceLanguage",
  "targetLanguage",
  "swapButton",
  "sourceText",
  "clearButton",
  "translateButton",
  "translatedText",
  "copyButton",
  "engineBadge",
  "modelBadge",
  "environmentNotice",
  "performanceStatus",
]) {
  if (!indexHtml.includes(`id="${id}"`)) {
    throw new Error(`Translator UI is missing #${id}`);
  }
}
if (!indexHtml.includes('<details class="technical-panel">')) {
  throw new Error("Technical diagnostics must remain collapsed below the translator");
}

const activeModel = APP_CONFIG.models[APP_CONFIG.activeModelId];
const fallbackModel = APP_CONFIG.models[APP_CONFIG.fallbackModelId];
const productGoalModel = APP_CONFIG.models[APP_CONFIG.productGoalModelId];
if (!activeModel || !fallbackModel || !productGoalModel) {
  throw new Error("Alpha model configuration references an unknown model");
}
if (activeModel.runtimeStatus !== "stable") {
  throw new Error(`Blocked model cannot be active: ${activeModel.id}`);
}
if (fallbackModel.runtimeStatus !== "stable") {
  throw new Error(`Fallback model must be stable: ${fallbackModel.id}`);
}
if (productGoalModel.runtimeStatus === "blocked" && productGoalModel.id === activeModel.id) {
  throw new Error("Blocked 440MB product target cannot become the active model");
}
for (const model of Object.values(APP_CONFIG.models)) {
  if (!Number.isSafeInteger(model.bytes) || model.bytes <= 0) {
    throw new Error(`Invalid model byte length: ${model.id}`);
  }
  if (!/^[a-f0-9]{64}$/.test(model.sha256)) {
    throw new Error(`Invalid model SHA-256: ${model.id}`);
  }
}
process.stdout.write("Alpha UI, model policy, static assets, and SHA-256 checks passed.\n");
