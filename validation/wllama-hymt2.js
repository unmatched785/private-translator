import { Wllama, LogLevel } from "/vendor/wllama/index.js";

const isStqValidation = new URLSearchParams(location.search).get("model") === "stq";
const model = isStqValidation
  ? {
      url: "/model/Hy-MT2-1.8B-1.25Bit.gguf",
      bytes: 461_860_800,
      label: "Hy-MT2-1.8B-1.25Bit",
    }
  : {
      url: "/model/Hy-MT2-1.8B-Q4_K_M.gguf",
      bytes: 1_133_080_448,
      label: "Hy-MT2-1.8B-Q4_K_M",
    };
const elements = {
  capabilities: document.querySelector("#capabilities"),
  run: document.querySelector("#run"),
  source: document.querySelector("#source"),
  status: document.querySelector("#status"),
  progress: document.querySelector("#progress"),
  progressLabel: document.querySelector("#progress-label"),
  output: document.querySelector("#output"),
  metrics: document.querySelector("#metrics"),
  log: document.querySelector("#log"),
};

let engine;

function formatBytes(bytes) {
  if (!Number.isFinite(bytes)) return "알 수 없음";
  return `${(bytes / 1024 / 1024).toFixed(1)}MB`;
}

function formatMs(milliseconds) {
  return `${Math.round(milliseconds).toLocaleString("ko-KR")}ms`;
}

function appendLog(message) {
  const timestamp = new Date().toLocaleTimeString("ko-KR", { hour12: false });
  elements.log.textContent += `[${timestamp}] ${message}\n`;
}

function renderDefinitionList(element, entries) {
  element.replaceChildren(
    ...entries.flatMap(([term, description]) => {
      const dt = document.createElement("dt");
      const dd = document.createElement("dd");
      dt.textContent = term;
      dd.textContent = description;
      return [dt, dd];
    }),
  );
}

function setStatus(message, type = "status") {
  elements.status.textContent = message;
  elements.status.className = type;
  appendLog(message);
}

async function renderCapabilities() {
  let adapterName = "미확인";
  if (navigator.gpu) {
    try {
      const adapter = await navigator.gpu.requestAdapter();
      adapterName = adapter?.info?.description || adapter?.info?.device || "사용 가능";
    } catch (error) {
      adapterName = `조회 실패: ${error.message}`;
    }
  }
  renderDefinitionList(elements.capabilities, [
    ["Chrome", navigator.userAgent],
    ["격리 모드", String(crossOriginIsolated)],
    ["WebGPU", navigator.gpu ? adapterName : "지원 안 함"],
    ["논리 CPU", String(navigator.hardwareConcurrency ?? "미확인")],
    ["wllama", "3.5.1 (로컬 설치본)"],
    ["모델", `${model.label} (${formatBytes(model.bytes)})`],
  ]);
}

function buildPrompt(text) {
  return [
    "Translate the following text from Korean into English. Note that you must ONLY output the translated result without any additional explanation:",
    "",
    text,
  ].join("\n");
}

async function loadEngine() {
  const logger = {
    debug: (...args) => console.debug("[wllama]", ...args),
    log: (...args) => console.log("[wllama]", ...args),
    warn: (...args) => console.warn("[wllama]", ...args),
    error: (...args) => console.error("[wllama]", ...args),
  };
  engine = new Wllama(
    {
      default: "/vendor/wllama/wasm/wllama.wasm",
      "single-thread/wllama.wasm": "/vendor/wllama/wasm/wllama.wasm",
      "multi-thread/wllama.wasm": "/vendor/wllama/wasm/wllama.wasm",
    },
    { logger, allowOffline: true, suppressNativeLog: false },
  );
  engine.setCompat(null);
  appendLog(`wllama WebGPU 지원 판정: ${engine.isSupportWebGPU()}`);

  const started = performance.now();
  await engine.loadModelFromUrl(model.url, {
    useCache: true,
    n_ctx: 2048,
    n_batch: 256,
    n_threads: Math.max(1, Math.min(4, Math.floor((navigator.hardwareConcurrency || 2) / 2))),
    n_gpu_layers: 999,
    flash_attn: true,
    seed: 42,
    log_level: LogLevel.INFO,
    progressCallback: ({ loaded, total }) => {
      elements.progress.max = total || model.bytes;
      elements.progress.value = loaded;
      elements.progressLabel.textContent = `${formatBytes(loaded)} / ${formatBytes(total || model.bytes)}`;
    },
  });
  return performance.now() - started;
}

async function translate(text) {
  const started = performance.now();
  const response = await engine.createChatCompletion({
    messages: [{ role: "user", content: buildPrompt(text) }],
    temperature: 0.7,
    top_p: 0.6,
    top_k: 20,
    penalty_repeat: 1.05,
    seed: 42,
    max_tokens: 512,
    stream: false,
  });
  return {
    text: response.choices[0]?.message?.content?.trim() ?? "",
    durationMs: performance.now() - started,
    usage: response.usage,
  };
}

elements.run.addEventListener("click", async () => {
  elements.run.disabled = true;
  elements.output.textContent = "실행 중…";
  elements.metrics.replaceChildren();
  try {
    let loadDurationMs = 0;
    if (!engine?.isModelLoaded()) {
      setStatus("로컬 모델을 Chrome으로 읽는 중…");
      loadDurationMs = await loadEngine();
      setStatus(`모델 로드 완료 (${formatMs(loadDurationMs)})`);
    }

    setStatus("Hy-MT2가 Chrome 안에서 번역 중…");
    const result = await translate(elements.source.value.trim());
    elements.output.textContent = result.text || "(빈 결과)";
    renderDefinitionList(elements.metrics, [
      ["모델 로드", loadDurationMs ? formatMs(loadDurationMs) : "이미 로드됨"],
      ["번역 생성", formatMs(result.durationMs)],
      ["토큰", result.usage ? `${result.usage.total_tokens}개` : "미확인"],
      ["네트워크", "추론 요청 없음 (모델은 최초 1회 정적 파일로 받음)"],
    ]);
    setStatus("성공: 백엔드 없이 Chrome 탭 내부에서 번역했습니다.");
    window.__WLLAMA_VALIDATION__ = {
      ok: true,
      input: elements.source.value,
      output: result.text,
      loadDurationMs,
      generationDurationMs: result.durationMs,
      usage: result.usage,
      context: engine.getLoadedContextInfo(),
    };
  } catch (error) {
    console.error(error);
    elements.output.textContent = `${error.name}: ${error.message}`;
    setStatus(`실패: ${error.message}`, "error");
    window.__WLLAMA_VALIDATION__ = {
      ok: false,
      name: error.name,
      message: error.message,
      stack: error.stack,
    };
  } finally {
    elements.run.disabled = false;
  }
});

renderCapabilities();
