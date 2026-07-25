# Hy-MT2 1.25-bit STQ compatibility

## Result

The official 440.5MiB model is not compatible with the product's current browser runtime.
It remains the product-size target, but it must not replace the verified Q4 fallback yet.

- Official revision: `9df5c824a00a744fb0512a29c640466f4d97dfb0`
- Size: `461860800` bytes
- SHA-256: `cc497fe8f033b52b3b8b00a7669e9661435432f9d4cd43f7ed24400c01507a93`
- Integrity: passed
- License: Apache-2.0

## GGUF inspection

`npm run alpha:inspect-stq` reads the GGUF header without loading model weights.

- GGUF version: `3`
- Architecture: `hunyuan-dense`
- Tokenizer: embedded GPT-2 / `hunyuan-dense` metadata and chat template
- Tensor count: `354`
- Tensor types: `129` type `0`, `1` type `14`, `224` type `42`
- Quantization metadata: `general.name = HyMT2 1.8B 2bit Stride16`
- File type: `41`

The tokenizer and general model metadata are present. The load failure begins when the
runtime reads the first type-42 weight, `blk.0.attn_k.weight`.

## Native llama.cpp

Pinned runtime `9966 (c749cb041)` rejected the STQ tensor layout:

`tensor 'blk.0.attn_k_norm.weight' has offset 203154464, expected 203277344`

The offset difference is a consequence of the runtime calculating the preceding type-42
tensor with a different or unsupported block layout. It is not evidence of a truncated file;
the exact byte size and SHA-256 both pass.

## Chrome wllama

Chrome 150 downloaded and cached all 440.5MiB in 4,993ms, but wllama 3.5.1 rejected the STQ tensor type:

`tensor 'blk.0.attn_k.weight' has invalid ggml type 42. should be in [0, 42)`

The subsequent chat call aborted because no model was loaded.

wllama 3.5.1 identifies its embedded llama.cpp as `b9640-dd4623a`. In that build,
`GGML_TYPE_COUNT` is `42`, so type `42` is outside the accepted range.

## Upstream dependency

The official model card states that this GGUF depends on llama.cpp PR `#22836`.
That PR adds `GGML_TYPE_STQ1_0 = 42` and changes `GGML_TYPE_COUNT` to `43`.
As of this recheck the PR is still open.

The PR changes CPU quantization code, including an ARM NEON vector-dot kernel and a
generic scalar fallback. It does not add a WebGPU STQ1_0 kernel or touch the WebGPU backend.
Therefore:

1. Upgrading to the latest released wllama does not solve the load failure; `3.5.1` is already
   the latest release and predates the unmerged STQ type.
2. Copying only the enum or tensor-size definition would allow parsing to progress but would
   not provide correct WebGPU inference.
3. A private browser fork would need both the upstream STQ changes and an audited WebGPU
   implementation, then cross-device correctness tests. That is a material runtime fork, not
   a narrow application patch.

## Decision

Do not use this model as the web default yet. A quality benchmark would be misleading because
the current production runtime cannot execute it.

- Product target: official 440.5MiB STQ model
- Current stable and rollback model: 1.13GB Q4
- Default switch gate: browser load, WebGPU offload, browser persistence/restart, matched
  quality benchmark, and major-error review must all pass

Reconsider after upstream STQ support includes a WebGPU path consumable by wllama, or after
the project explicitly accepts the cost of maintaining and auditing a compatible browser fork.
