export const APP_CONFIG = Object.freeze({
  productGoalModelId: "hymt2-stq1",
  activeModelId: "hymt2-q4",
  fallbackModelId: "hymt2-q4",
  links: Object.freeze({
    repository: "https://github.com/unmatched785/private-translator",
    feedback:
      "https://github.com/unmatched785/private-translator/issues/new?template=alpha-feedback.yml",
  }),
  models: Object.freeze({
    "hymt2-stq1": Object.freeze({
      id: "hymt2-stq1",
      label: "Hy-MT2 1.8B 1.25-bit",
      shortLabel: "440MB 목표 모델",
      fileName: "Hy-MT2-1.8B-1.25Bit.gguf",
      bytes: 461_860_800,
      revision: "9df5c824a00a744fb0512a29c640466f4d97dfb0",
      sha256: "cc497fe8f033b52b3b8b00a7669e9661435432f9d4cd43f7ed24400c01507a93",
      repository: "tencent/Hy-MT2-1.8B-1.25Bit-GGUF",
      url: "https://huggingface.co/tencent/Hy-MT2-1.8B-1.25Bit-GGUF/resolve/9df5c824a00a744fb0512a29c640466f4d97dfb0/Hy-MT2-1.8B-1.25Bit.gguf?download=true",
      localUrl: "/model/Hy-MT2-1.8B-1.25Bit.gguf",
      runtimeStatus: "blocked",
      runtimeReason:
        "STQ1_0(type 42) 지원이 아직 llama.cpp upstream과 wllama WebGPU에 포함되지 않았습니다.",
      license: "Apache-2.0",
    }),
    "hymt2-q4": Object.freeze({
      id: "hymt2-q4",
      label: "Hy-MT2 1.8B Q4",
      shortLabel: "1.13GB 안정 모델",
      fileName: "Hy-MT2-1.8B-Q4_K_M.gguf",
      bytes: 1_133_080_448,
      revision: "1cd5208700acedef4ef93019b6cfc148b8522d45",
      sha256: "dc5f44fcf1fa496ee7ad725982c0c8c553a4de00259b53af84c4b89fb0c06699",
      repository: "tencent/Hy-MT2-1.8B-GGUF",
      url: "https://huggingface.co/tencent/Hy-MT2-1.8B-GGUF/resolve/1cd5208700acedef4ef93019b6cfc148b8522d45/Hy-MT2-1.8B-Q4_K_M.gguf?download=true",
      localUrl: "/model/Hy-MT2-1.8B-Q4_K_M.gguf",
      runtimeStatus: "stable",
      runtimeReason: "현재 Chrome/wllama/WebGPU 검증을 통과한 롤백 가능 모델입니다.",
      license: "Apache-2.0",
    }),
  }),
});
