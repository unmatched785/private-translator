import { LogLevel, Wllama } from "/vendor/wllama/index.js";
import { sha256Stream } from "/sha256.js";

const MODEL = Object.freeze({
  fileName: "Hy-MT2-1.8B-Q4_K_M.gguf",
  bytes: 1_133_080_448,
  revision: "1cd5208700acedef4ef93019b6cfc148b8522d45",
  sha256: "dc5f44fcf1fa496ee7ad725982c0c8c553a4de00259b53af84c4b89fb0c06699",
  url: "https://huggingface.co/tencent/Hy-MT2-1.8B-GGUF/resolve/1cd5208700acedef4ef93019b6cfc148b8522d45/Hy-MT2-1.8B-Q4_K_M.gguf?download=true",
});

const MODEL_FOLDER = "private-translator";
const MODEL_METADATA_KEY = "private-translator.model.v1";
const LOAD_METADATA_KEY = "private-translator.last-load.v1";
const isLocalModelSource = new URLSearchParams(location.search).get("source") === "local";

const elements = Object.fromEntries(
  [
    "modelBadge",
    "compatibility",
    "storageStatus",
    "downloadProgress",
    "downloadLabel",
    "downloadEta",
    "installButton",
    "pauseButton",
    "modelFileInput",
    "exportButton",
    "engineBadge",
    "sourceLanguage",
    "targetLanguage",
    "swapButton",
    "sourceText",
    "characterCount",
    "translateButton",
    "copyButton",
    "translatedText",
    "timingLine",
  ].map((id) => [id, document.querySelector(`#${id}`)]),
);

const state = {
  compatible: false,
  activeFile: null,
  activeSource: null,
  downloadController: null,
  engine: null,
  enginePromise: null,
  loadDurationMs: 0,
  busy: false,
};

function formatBytes(bytes, digits = 1) {
  if (!Number.isFinite(bytes)) return "알 수 없음";
  if (bytes >= 1024 ** 3) return `${(bytes / 1024 ** 3).toFixed(2)}GB`;
  return `${(bytes / 1024 ** 2).toFixed(digits)}MB`;
}

function formatDuration(seconds) {
  if (!Number.isFinite(seconds) || seconds < 0) return "";
  if (seconds < 60) return `약 ${Math.max(1, Math.round(seconds))}초 남음`;
  return `약 ${Math.ceil(seconds / 60)}분 남음`;
}

function formatMilliseconds(milliseconds) {
  return `${Math.round(milliseconds).toLocaleString("ko-KR")}ms`;
}

function setBadge(element, label, status) {
  element.textContent = label;
  element.dataset.state = status;
}

function setDownloadProgress(value, label, eta = "") {
  elements.downloadProgress.value = Math.min(value, MODEL.bytes);
  elements.downloadLabel.textContent = label;
  elements.downloadEta.textContent = eta;
}

function readVerifiedMetadata() {
  try {
    return JSON.parse(localStorage.getItem(MODEL_METADATA_KEY) || "null");
  } catch {
    return null;
  }
}

function metadataMatches(metadata) {
  return (
    metadata?.fileName === MODEL.fileName &&
    metadata?.bytes === MODEL.bytes &&
    metadata?.revision === MODEL.revision &&
    metadata?.sha256 === MODEL.sha256
  );
}

function writeVerifiedMetadata(source) {
  localStorage.setItem(
    MODEL_METADATA_KEY,
    JSON.stringify({
      fileName: MODEL.fileName,
      bytes: MODEL.bytes,
      revision: MODEL.revision,
      sha256: MODEL.sha256,
      source,
      verifiedAt: new Date().toISOString(),
    }),
  );
}

async function getModelDirectory() {
  const root = await navigator.storage.getDirectory();
  return root.getDirectoryHandle(MODEL_FOLDER, { create: true });
}

async function getCachedModelFile() {
  const directory = await getModelDirectory();
  const handle = await directory.getFileHandle(MODEL.fileName);
  return handle.getFile();
}

async function getCachedFileSize() {
  try {
    return (await getCachedModelFile()).size;
  } catch (error) {
    if (error.name === "NotFoundError") return 0;
    throw error;
  }
}

async function deleteCachedModel() {
  try {
    const directory = await getModelDirectory();
    await directory.removeEntry(MODEL.fileName);
  } catch (error) {
    if (error.name !== "NotFoundError") throw error;
  }
  localStorage.removeItem(MODEL_METADATA_KEY);
}

function activateModel(file, source) {
  state.activeFile = file;
  state.activeSource = source;
  setBadge(elements.modelBadge, "사용 가능", "ready");
  setBadge(elements.engineBadge, "열기 전", "ready");
  elements.sourceText.disabled = false;
  elements.translateButton.disabled = !elements.sourceText.value.trim();
  elements.exportButton.hidden = source !== "browser";
}

function deactivateModel() {
  state.activeFile = null;
  state.activeSource = null;
  elements.sourceText.disabled = true;
  elements.translateButton.disabled = true;
  elements.exportButton.hidden = true;
  setBadge(elements.engineBadge, "모델 필요", "idle");
}

async function renderStorageStatus() {
  const [persisted, estimate] = await Promise.all([
    navigator.storage.persisted(),
    navigator.storage.estimate(),
  ]);
  const used = formatBytes(estimate.usage || 0);
  const quota = formatBytes(estimate.quota || 0);
  elements.storageStatus.textContent = persisted
    ? `저장소 보호 요청 승인됨 · 현재 ${used} / ${quota}`
    : `자동 정리 보호 미승인 · 현재 ${used} / ${quota} · 설치할 때 보호를 요청합니다.`;
  return persisted;
}

async function requestPersistentStorage() {
  try {
    if (!(await navigator.storage.persisted())) await navigator.storage.persist();
  } finally {
    await renderStorageStatus();
  }
}

async function inspectCachedModel() {
  const fileSize = await getCachedFileSize();
  const metadata = readVerifiedMetadata();

  if (fileSize === MODEL.bytes && metadataMatches(metadata)) {
    const file = await getCachedModelFile();
    activateModel(file, "browser");
    setDownloadProgress(MODEL.bytes, `검증된 모델 준비됨 · ${formatBytes(MODEL.bytes)}`);
    elements.installButton.textContent = "설치 완료";
    elements.installButton.disabled = true;
    return;
  }

  deactivateModel();
  setBadge(elements.modelBadge, fileSize ? "부분 저장" : "설치 필요", fileSize ? "paused" : "idle");
  elements.installButton.textContent =
    fileSize === MODEL.bytes
      ? "완료 파일 확인"
      : fileSize
        ? "다운로드 이어받기"
        : "1.13GB 모델 설치";
  elements.installButton.disabled = !state.compatible;

  if (fileSize > MODEL.bytes) {
    await deleteCachedModel();
    setDownloadProgress(0, "유효하지 않은 부분 파일을 제거했습니다.");
    return;
  }

  setDownloadProgress(
    fileSize,
    fileSize
      ? `${formatBytes(fileSize)} 보존됨 · 다음 설치에서 이어받습니다.`
      : "아직 다운로드하지 않았습니다.",
  );
}

async function hashFile(file, label = "무결성 확인 중") {
  const started = performance.now();
  let processed = 0;
  const reader = file.stream().getReader();
  const monitoredStream = new ReadableStream({
    async pull(controller) {
      const { done, value } = await reader.read();
      if (done) {
        controller.close();
        return;
      }
      processed += value.byteLength;
      setDownloadProgress(
        processed,
        `${label} · ${formatBytes(processed)} / ${formatBytes(file.size)}`,
      );
      controller.enqueue(value);
    },
    cancel(reason) {
      return reader.cancel(reason);
    },
  });
  const hash = await sha256Stream(monitoredStream);
  return { hash, durationMs: performance.now() - started };
}

async function verifyModelFile(file, source) {
  if (file.size !== MODEL.bytes) {
    throw new Error(`파일 크기가 다릅니다. ${formatBytes(file.size)} / ${formatBytes(MODEL.bytes)}`);
  }

  setBadge(elements.modelBadge, "검증 중", "checking");
  const verification = await hashFile(file);
  if (verification.hash !== MODEL.sha256) {
    throw new Error(`SHA-256 불일치 (${verification.hash.slice(0, 12)}…)`);
  }

  if (source === "browser") writeVerifiedMetadata("download");
  activateModel(file, source);
  setDownloadProgress(
    MODEL.bytes,
    `SHA-256 확인 완료 · ${formatMilliseconds(verification.durationMs)}`,
  );
  elements.installButton.textContent = source === "browser" ? "설치 완료" : "브라우저에도 설치";
  elements.installButton.disabled = source === "browser";
}

async function openWritableAtOffset(handle, offset) {
  const writable = await handle.createWritable({ keepExistingData: true });
  if (offset === 0) await writable.truncate(0);
  await writable.seek(offset);
  return writable;
}

async function downloadModel() {
  if (state.downloadController || state.busy) return;
  state.busy = true;
  await requestPersistentStorage();

  const directory = await getModelDirectory();
  const handle = await directory.getFileHandle(MODEL.fileName, { create: true });
  let offset = (await handle.getFile()).size;
  if (offset > MODEL.bytes) {
    const reset = await handle.createWritable();
    await reset.truncate(0);
    await reset.close();
    offset = 0;
  }

  if (offset === MODEL.bytes) {
    elements.installButton.disabled = true;
    try {
      await verifyModelFile(await handle.getFile(), "browser");
    } catch (error) {
      console.error(error);
      await deleteCachedModel();
      deactivateModel();
      setBadge(elements.modelBadge, "파일 거부", "error");
      setDownloadProgress(0, `GGUF 확인 실패: ${error.message}`);
      elements.installButton.textContent = "1.13GB 모델 다시 설치";
      elements.installButton.disabled = !state.compatible;
    } finally {
      state.busy = false;
      await renderStorageStatus();
    }
    return;
  }

  const controller = new AbortController();
  state.downloadController = controller;
  elements.installButton.disabled = true;
  elements.pauseButton.hidden = false;
  setBadge(elements.modelBadge, "받는 중", "downloading");

  let writable;
  const started = performance.now();
  const initialOffset = offset;

  try {
    const modelUrl = isLocalModelSource ? "/model/Hy-MT2-1.8B-Q4_K_M.gguf" : MODEL.url;
    const headers = offset ? { Range: `bytes=${offset}-` } : {};
    let response = await fetch(modelUrl, {
      headers,
      cache: "no-store",
      signal: controller.signal,
    });

    if (offset && response.status !== 206) {
      response.body?.cancel();
      offset = 0;
      response = await fetch(modelUrl, { cache: "no-store", signal: controller.signal });
    }
    if (!response.ok || !response.body) {
      throw new Error(`모델 다운로드 실패 (HTTP ${response.status})`);
    }

    writable = await openWritableAtOffset(handle, offset);
    const reader = response.body.getReader();
    let received = offset;
    let lastRender = 0;

    while (true) {
      const { done, value } = await reader.read();
      if (done) break;
      await writable.write(value);
      received += value.byteLength;

      const now = performance.now();
      if (now - lastRender >= 150 || received === MODEL.bytes) {
        const elapsedSeconds = Math.max((now - started) / 1000, 0.001);
        const speed = (received - initialOffset) / elapsedSeconds;
        setDownloadProgress(
          received,
          `${formatBytes(received)} / ${formatBytes(MODEL.bytes)} · ${formatBytes(speed, 1)}/s`,
          formatDuration((MODEL.bytes - received) / speed),
        );
        lastRender = now;
      }
    }
    await writable.close();
    writable = null;

    const file = await handle.getFile();
    if (file.size !== MODEL.bytes) {
      throw new Error(
        `다운로드가 일찍 끝났습니다. ${formatBytes(file.size)}까지 보존했으니 다시 이어받아 주세요.`,
      );
    }
    await verifyModelFile(file, "browser");
  } catch (error) {
    if (writable) {
      try {
        await writable.close();
      } catch {
        // Closing an aborted OPFS stream can fail; the partial file remains reusable.
      }
    }

    if (error.name === "AbortError") {
      const saved = await getCachedFileSize();
      setBadge(elements.modelBadge, "일시중지", "paused");
      setDownloadProgress(saved, `${formatBytes(saved)} 보존됨 · 이어받을 수 있습니다.`);
      elements.installButton.textContent = "다운로드 이어받기";
      return;
    }

    console.error(error);
    const saved = await getCachedFileSize();
    const corrupt = saved >= MODEL.bytes;
    if (corrupt) await deleteCachedModel();
    deactivateModel();
    setBadge(elements.modelBadge, "실패", "error");
    setDownloadProgress(
      corrupt ? 0 : saved,
      corrupt
        ? `설치 실패: ${error.message}`
        : `설치 중단: ${error.message} · ${formatBytes(saved)} 보존됨`,
    );
  } finally {
    state.downloadController = null;
    state.busy = false;
    elements.pauseButton.hidden = true;
    if (!state.activeFile) elements.installButton.disabled = !state.compatible;
    await renderStorageStatus();
  }
}

async function useExistingFile(file) {
  if (state.busy || !file) return;
  state.busy = true;
  elements.modelFileInput.disabled = true;
  try {
    await verifyModelFile(file, "file");
  } catch (error) {
    console.error(error);
    deactivateModel();
    setBadge(elements.modelBadge, "파일 거부", "error");
    setDownloadProgress(0, `GGUF 확인 실패: ${error.message}`);
  } finally {
    state.busy = false;
    elements.modelFileInput.disabled = false;
    elements.modelFileInput.value = "";
  }
}

async function exportCachedModel() {
  const file = await getCachedModelFile();
  const url = URL.createObjectURL(file);
  const anchor = document.createElement("a");
  anchor.href = url;
  anchor.download = MODEL.fileName;
  anchor.click();
  setTimeout(() => URL.revokeObjectURL(url), 60_000);
}

function buildPrompt(text, source, target) {
  const languageNames = {
    ko: "Korean",
    en: "English",
  };
  return [
    `Translate the following text from ${languageNames[source]} into ${languageNames[target]}.`,
    "You must ONLY output the translated result without any explanation:",
    "",
    text,
  ].join("\n");
}

async function loadEngine() {
  if (state.engine?.isModelLoaded()) return state.engine;
  if (state.enginePromise) return state.enginePromise;
  if (!state.activeFile) throw new Error("검증된 모델이 없습니다.");

  setBadge(elements.engineBadge, "모델 여는 중", "checking");
  elements.timingLine.textContent = "인터넷 다운로드가 아니라 로컬 모델을 메모리에 올리는 중입니다.";
  const file = state.activeFile;
  const source = state.activeSource;

  state.enginePromise = (async () => {
    const started = performance.now();
    const engine = new Wllama(
      {
        default: "/vendor/wllama/wasm/wllama.wasm",
        "single-thread/wllama.wasm": "/vendor/wllama/wasm/wllama.wasm",
        "multi-thread/wllama.wasm": "/vendor/wllama/wasm/wllama.wasm",
      },
      {
        allowOffline: true,
        suppressNativeLog: false,
      },
    );
    engine.setCompat(null);
    await engine.loadModel([file], {
      n_ctx: 2048,
      n_batch: 256,
      n_threads: Math.max(1, Math.min(4, Math.floor((navigator.hardwareConcurrency || 2) / 2))),
      n_gpu_layers: 999,
      flash_attn: true,
      seed: 42,
      log_level: LogLevel.WARN,
    });

    state.engine = engine;
    state.loadDurationMs = performance.now() - started;
    localStorage.setItem(
      LOAD_METADATA_KEY,
      JSON.stringify({
        measuredAt: new Date().toISOString(),
        durationMs: state.loadDurationMs,
        source,
      }),
    );
    setBadge(elements.engineBadge, "준비됨", "ready");
    return engine;
  })();

  try {
    return await state.enginePromise;
  } catch (error) {
    state.engine = null;
    setBadge(elements.engineBadge, "열기 실패", "error");
    throw error;
  } finally {
    state.enginePromise = null;
  }
}

async function translate() {
  const text = elements.sourceText.value.trim();
  if (!text || state.busy) return;
  state.busy = true;
  elements.translateButton.disabled = true;
  elements.translatedText.textContent = "로컬 모델을 준비하고 있습니다…";
  elements.copyButton.disabled = true;

  try {
    const engine = await loadEngine();
    elements.translatedText.textContent = "이 기기에서 번역 중…";
    const started = performance.now();
    const response = await engine.createChatCompletion({
      messages: [
        {
          role: "user",
          content: buildPrompt(
            text,
            elements.sourceLanguage.value,
            elements.targetLanguage.value,
          ),
        },
      ],
      temperature: 0.7,
      top_p: 0.6,
      top_k: 20,
      penalty_repeat: 1.05,
      seed: 42,
      max_tokens: 1024,
      stream: false,
    });
    const generationDuration = performance.now() - started;
    const result = response.choices[0]?.message?.content?.trim() || "";
    elements.translatedText.textContent = result || "(빈 결과)";
    elements.copyButton.disabled = !result;
    elements.timingLine.textContent =
      `로컬 모델 열기 ${formatMilliseconds(state.loadDurationMs)} · ` +
      `이번 번역 ${formatMilliseconds(generationDuration)} · 원문/결과 네트워크 전송 없음`;
  } catch (error) {
    console.error(error);
    elements.translatedText.textContent = `번역 실패: ${error.message}`;
    elements.timingLine.textContent = "민감한 원문을 피드백에 붙이지 말고 실행 환경만 알려 주세요.";
  } finally {
    state.busy = false;
    elements.translateButton.disabled = !state.activeFile || !elements.sourceText.value.trim();
  }
}

function swapDirection() {
  const source = elements.sourceLanguage.value;
  elements.sourceLanguage.value = elements.targetLanguage.value;
  elements.targetLanguage.value = source;
}

async function checkCompatibility() {
  const isChrome = /Chrome\//.test(navigator.userAgent) && !/Edg\//.test(navigator.userAgent);
  const secureEnough = window.isSecureContext;
  const hasWebGpu = Boolean(navigator.gpu);
  const hasOpfs = Boolean(navigator.storage?.getDirectory);
  state.compatible = isChrome && secureEnough && hasWebGpu && hasOpfs && crossOriginIsolated;

  const checks = [
    ["Chrome", isChrome],
    ["WebGPU", hasWebGpu],
    ["브라우저 파일 저장", hasOpfs],
    ["격리 모드", crossOriginIsolated],
  ];
  const summary = checks.map(([label, ok]) => `${ok ? "✓" : "✕"} ${label}`).join(" · ");
  elements.compatibility.textContent = state.compatible
    ? `${summary} · 이 기기에서 알파를 실행할 수 있습니다.`
    : `${summary} · 최신 데스크톱 Chrome과 HTTPS 환경이 필요합니다.`;
  elements.compatibility.dataset.state = state.compatible ? "ready" : "error";
}

elements.installButton.addEventListener("click", downloadModel);
elements.pauseButton.addEventListener("click", () => state.downloadController?.abort());
elements.modelFileInput.addEventListener("change", (event) => useExistingFile(event.target.files?.[0]));
elements.exportButton.addEventListener("click", exportCachedModel);
elements.translateButton.addEventListener("click", translate);
elements.swapButton.addEventListener("click", swapDirection);
elements.sourceLanguage.addEventListener("change", () => {
  elements.targetLanguage.value = elements.sourceLanguage.value === "ko" ? "en" : "ko";
});
elements.targetLanguage.addEventListener("change", () => {
  elements.sourceLanguage.value = elements.targetLanguage.value === "ko" ? "en" : "ko";
});
elements.sourceText.addEventListener("input", () => {
  elements.characterCount.textContent =
    `${elements.sourceText.value.length.toLocaleString("ko-KR")} / 6,000`;
  elements.translateButton.disabled = !state.activeFile || !elements.sourceText.value.trim() || state.busy;
});
elements.copyButton.addEventListener("click", async () => {
  await navigator.clipboard.writeText(elements.translatedText.textContent);
  elements.copyButton.textContent = "복사됨";
  setTimeout(() => {
    elements.copyButton.textContent = "복사";
  }, 1_200);
});

window.addEventListener("beforeunload", () => state.downloadController?.abort());

async function boot() {
  if ("serviceWorker" in navigator) {
    navigator.serviceWorker.register("/sw.js").catch((error) => {
      console.warn("Service worker registration failed", error);
    });
  }

  try {
    await checkCompatibility();
    await renderStorageStatus();
    await inspectCachedModel();
    const lastLoad = (() => {
      try {
        return JSON.parse(localStorage.getItem(LOAD_METADATA_KEY) || "null");
      } catch {
        return null;
      }
    })();
    if (lastLoad?.durationMs) {
      elements.timingLine.textContent =
        `마지막 로컬 모델 열기 ${formatMilliseconds(lastLoad.durationMs)} · ` +
        `${new Date(lastLoad.measuredAt).toLocaleString("ko-KR")}`;
    }
  } catch (error) {
    console.error(error);
    setBadge(elements.modelBadge, "확인 실패", "error");
    elements.compatibility.textContent = `브라우저 저장소 확인 실패: ${error.message}`;
  }
}

boot();
