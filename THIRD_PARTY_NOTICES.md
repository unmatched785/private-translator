# Third-party notices

Private Translator does not send translation text to these projects. They run as local components.

## Tencent Hy-MT2 1.8B GGUF

- Source: https://huggingface.co/tencent/Hy-MT2-1.8B-GGUF
- File: `Hy-MT2-1.8B-Q4_K_M.gguf`
- Pinned revision: `1cd5208700acedef4ef93019b6cfc148b8522d45`
- License: Apache License 2.0
- Copyright belongs to the model authors and contributors.

The v0.1.2 distribution does not contain the GGUF file. Explicit setup downloads it from the pinned official source and verifies its size and SHA-256. The distribution folder includes the upstream model license as `licenses/Hy-MT2-LICENSE`, fetched from the same pinned model revision and verified as 11,639 bytes with SHA-256 `a1d52d448f81c584a47c583e19dfab2d3851c7c84431b07baee093c1113ed114`.

## llama.cpp

- Source: https://github.com/ggml-org/llama.cpp
- Release: b9966
- Commit: `c749cb041706647f460bb918cccc9d91995205ab`
- License: MIT
- Copyright belongs to the llama.cpp authors and contributors.

The distribution folder includes the upstream license as `licenses/llama.cpp-LICENSE`, fetched from that exact commit and verified as 1,078 bytes with SHA-256 `94f29bbed6a22c35b992c5c6ebf0e7c92f13b836b90f36f461c9cf2f0f1d010d`.

## Rust dependencies

The Windows executable statically links Rust crates resolved by the committed `Cargo.lock`. Every release archive includes:

- `supply-chain/rust-dependency-licenses.json`, generated from `cargo metadata --locked` for `x86_64-pc-windows-msvc`;
- `supply-chain/private-translator.cdx.json`, a CycloneDX 1.5 software bill of materials;
- `supply-chain/licenses/rust/<crate>-<version>/`, containing the actual license, notice, or copying files shipped by each direct and transitive crate.

Release creation fails if a resolved Rust dependency has no SPDX license expression or no upstream license text in its packaged crate source.
