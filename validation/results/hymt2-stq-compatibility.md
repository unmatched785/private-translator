# Hy-MT2 1.25-bit STQ compatibility

## Result

The official 440.5MiB model is not compatible with the product's current runtimes.

- Official revision: `9df5c824a00a744fb0512a29c640466f4d97dfb0`
- Size: `461860800` bytes
- SHA-256: `cc497fe8f033b52b3b8b00a7669e9661435432f9d4cd43f7ed24400c01507a93`
- Integrity: passed
- License: Apache-2.0

## Native llama.cpp

Pinned runtime `9966 (c749cb041)` rejected the STQ tensor layout:

`tensor 'blk.0.attn_k_norm.weight' has offset 203154464, expected 203277344`

## Chrome wllama

Chrome 150 downloaded and cached all 440.5MiB in 4,993ms, but wllama 3.5.1 rejected the STQ tensor type:

`tensor 'blk.0.attn_k.weight' has invalid ggml type 42. should be in [0, 42)`

The subsequent chat call aborted because no model was loaded.

## Decision

Do not use this model as the web default yet. A quality benchmark would be misleading because the current production runtime cannot execute it. Reconsider only after wllama includes the required STQ kernel, or after maintaining and auditing a compatible browser runtime fork.
