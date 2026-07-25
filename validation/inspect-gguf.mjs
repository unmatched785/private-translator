import { open } from "node:fs/promises";
import { resolve } from "node:path";

const input = resolve(process.argv[2] ?? "models/Hy-MT2-1.8B-1.25Bit.gguf");
const file = await open(input, "r");
let position = 0;

async function read(length) {
  const buffer = Buffer.allocUnsafe(length);
  const { bytesRead } = await file.read(buffer, 0, length, position);
  if (bytesRead !== length) throw new Error(`Unexpected end of GGUF at byte ${position}`);
  position += length;
  return buffer;
}

async function uint32() {
  return (await read(4)).readUInt32LE();
}

async function uint64() {
  return (await read(8)).readBigUInt64LE();
}

async function string() {
  const length = Number(await uint64());
  return (await read(length)).toString("utf8");
}

async function scalar(type) {
  const readers = {
    0: async () => (await read(1)).readUInt8(),
    1: async () => (await read(1)).readInt8(),
    2: async () => (await read(2)).readUInt16LE(),
    3: async () => (await read(2)).readInt16LE(),
    4: uint32,
    5: async () => (await read(4)).readInt32LE(),
    6: async () => (await read(4)).readFloatLE(),
    7: async () => Boolean((await read(1)).readUInt8()),
    8: string,
    10: uint64,
    11: async () => (await read(8)).readBigInt64LE(),
    12: async () => (await read(8)).readDoubleLE(),
  };
  const reader = readers[type];
  if (!reader) throw new Error(`Unsupported GGUF metadata type ${type}`);
  return reader();
}

try {
  const magic = (await read(4)).toString("ascii");
  const version = await uint32();
  const tensorCount = await uint64();
  const metadataCount = await uint64();
  if (magic !== "GGUF") throw new Error(`Unexpected magic ${magic}`);

  const metadata = {};
  for (let index = 0n; index < metadataCount; index += 1n) {
    const key = await string();
    const type = await uint32();
    if (type !== 9) {
      metadata[key] = await scalar(type);
      continue;
    }

    const elementType = await uint32();
    const length = await uint64();
    if (length <= 64n) {
      const values = [];
      for (let item = 0n; item < length; item += 1n) values.push(await scalar(elementType));
      metadata[key] = values;
    } else {
      for (let item = 0n; item < length; item += 1n) await scalar(elementType);
      metadata[key] = `[${length} values]`;
    }
  }

  const tensorTypes = {};
  const firstByType = {};
  for (let index = 0n; index < tensorCount; index += 1n) {
    const name = await string();
    const dimensionCount = await uint32();
    const dimensions = [];
    for (let dimension = 0; dimension < dimensionCount; dimension += 1) {
      dimensions.push((await uint64()).toString());
    }
    const type = await uint32();
    const offset = await uint64();
    tensorTypes[type] = (tensorTypes[type] ?? 0) + 1;
    firstByType[type] ??= { name, dimensions, offset: offset.toString() };
  }

  const selectedMetadata = Object.fromEntries(
    Object.entries(metadata).filter(([key]) =>
      /^(general\.|tokenizer\.(ggml\.(model|pre)|chat_template))/.test(key),
    ),
  );
  process.stdout.write(
    `${JSON.stringify(
      {
        file: input,
        magic,
        version,
        tensorCount: Number(tensorCount),
        metadataCount: Number(metadataCount),
        metadata: selectedMetadata,
        tensorTypes,
        firstTensorByType: firstByType,
      },
      (_, value) => (typeof value === "bigint" ? value.toString() : value),
      2,
    )}\n`,
  );
} finally {
  await file.close();
}
