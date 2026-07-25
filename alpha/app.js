import { LogLevel, Wllama } from "/vendor/wllama/index.js";
import { APP_CONFIG } from "/config.js";
import { sha256Stream } from "/sha256.js";

const MODEL = APP_CONFIG.models[APP_CONFIG.activeModelId];
const PRODUCT_GOAL_MODEL = APP_CONFIG.models[APP_CONFIG.productGoalModelId];

const MODEL_FOLDER = "private-translator";
const MODEL_METADATA_KEY = "private-translator.model.v1";
const LOAD_METADATA_KEY = "private-translator.last-load.v1";
const isLocalModelSource = new URLSearchParams(location.search).get("source") === "local";

const elements = Object.fromEntries(
  [
    "modelBadge",
    "modelSetup",
    "modelSetupCopy",
    "installActions",
    "environmentNotice",
    "compatibility",
    "storageStatus",
    "performanceStatus",
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
    "clearButton",
    "characterCount",
    "translateButton",
    "copyButton",
    "translatedText",
    "timingLine",
    "feedbackLink",
    "repositoryLink",
    "modelRepository",
    "modelRevision",
    "modelBytes",
    "modelSha256",
    "modelLicense",
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
  translating: false,
  pendingTranslation: false,
};

let bootInteractionDetected = false;
window.addEventListener("pointerdown", () => {
  bootInteractionDetected = true;
}, { capture: true, once: true });
window.addEventListener("keydown", () => {
  bootInteractionDetected = true;
}, { capture: true, once: true });

function formatBytes(bytes, digits = 1) {
  if (!Number.isFinite(bytes)) return "알 수 없음";
  if (bytes >= 1_000_000_000) return `${(bytes / 1_000_000_000).toFixed(2)}GB`;
  return `${(bytes / 1_000_000).toFixed(digits)}MB`;
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

function refreshTranslateButton() {
  const hasText = Boolean(elements.sourceText.value.trim());
  elements.translateButton.textContent = state.translating ? "번역 중…" : "번역";
  elements.translateButton.disabled = !hasText || state.busy || !state.compatible;
  elements.clearButton.disabled = state.translating || !elements.sourceText.value;
}

function lockTranslationControls(locked) {
  elements.sourceText.readOnly = locked;
  elements.sourceLanguage.disabled = locked;
  elements.targetLanguage.disabled = locked;
  elements.swapButton.disabled = locked;
}

function resetResult(message = "번역 결과가 여기에 표시됩니다.") {
  elements.translatedText.textContent = message;
  elements.translatedText.setAttribute("aria-busy", "false");
  elements.timingLine.textContent = "";
  elements.copyButton.disabled = true;
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
  setBadge(
    elements.engineBadge,
    state.compatible ? "로컬 준비됨" : "환경 확인 필요",
    state.compatible ? "ready" : "error",
  );
  elements.modelSetup.classList.add("is-ready");
  elements.modelSetupCopy.textContent =
    source === "browser"
      ? "이 브라우저에 저장된 Q4 안정 모델이 준비됐습니다."
      : "선택한 Q4 모델 파일을 현재 탭에서 사용합니다. 다음 방문에는 파일을 다시 열어 주세요.";
  elements.installActions.hidden = true;
  refreshTranslateButton();
  elements.exportButton.hidden = source !== "browser";
}

function deactivateModel() {
  state.activeFile = null;
  state.activeSource = null;
  elements.modelSetup.classList.remove("is-ready");
  elements.modelSetupCopy.textContent =
    "현재 안정 모델을 한 번 설치하면 다음부터 저장된 파일을 다시 사용합니다.";
  elements.installActions.hidden = false;
  refreshTranslateButton();
  elements.exportButton.hidden = true;
  setBadge(
    elements.engineBadge,
    state.compatible ? "모델 필요" : "환경 확인 필요",
    state.compatible ? "idle" : "error",
  );
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
        : "1.13GB 안정 모델 설치";
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
  elements.installButton.textContent = source === "browser" ? "설치 완료" : "1.13GB 안정 모델 설치";
  elements.installButton.disabled = true;
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
  refreshTranslateButton();
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
      refreshTranslateButton();
      await renderStorageStatus();
    }
    await resumePendingTranslation();
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
    const modelUrl = isLocalModelSource ? MODEL.localUrl : MODEL.url;
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
    await resumePendingTranslation();
  }
}

async function useExistingFile(file) {
  if (state.busy || !file) return;
  state.busy = true;
  refreshTranslateButton();
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
    await resumePendingTranslation();
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
  elements.timingLine.textContent =
    "로컬 모델을 여는 중입니다. 첫 번역은 잠시 걸릴 수 있습니다.";
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
      n_threads: crossOriginIsolated
        ? Math.max(1, Math.min(4, Math.floor((navigator.hardwareConcurrency || 2) / 2)))
        : 1,
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
    elements.performanceStatus.textContent =
      `최근 모델 열기 ${formatMilliseconds(state.loadDurationMs)} · ` +
      `${new Date().toLocaleString("ko-KR")}`;
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
  if (!state.compatible) {
    elements.environmentNotice.hidden = false;
    elements.environmentNotice.scrollIntoView({ behavior: "smooth", block: "nearest" });
    return;
  }
  if (!state.activeFile) {
    state.pendingTranslation = true;
    elements.translatedText.textContent =
      "번역할 텍스트는 준비됐습니다. 아래에서 1.13GB 안정 모델을 한 번 설치해 주세요.";
    elements.timingLine.textContent =
      "440MB 목표 모델은 STQ WebGPU 지원 전까지 활성화하지 않습니다.";
    elements.modelSetup.scrollIntoView({ behavior: "smooth", block: "center" });
    elements.installButton.textContent = "1.13GB 모델 받고 번역";
    elements.installButton.focus({ preventScroll: true });
    return;
  }

  const sourceLanguage = elements.sourceLanguage.value;
  const targetLanguage = elements.targetLanguage.value;
  state.pendingTranslation = false;
  state.busy = true;
  state.translating = true;
  lockTranslationControls(true);
  refreshTranslateButton();
  elements.translatedText.textContent = "로컬 모델을 준비하고 있습니다…";
  elements.translatedText.setAttribute("aria-busy", "true");
  elements.copyButton.disabled = true;

  try {
    const engine = await loadEngine();
    elements.translatedText.textContent = "이 기기에서 번역 중…";
    const started = performance.now();
    const response = await engine.createChatCompletion({
      messages: [
        {
          role: "user",
          content: buildPrompt(text, sourceLanguage, targetLanguage),
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
    elements.translatedText.setAttribute("aria-busy", "false");
    elements.copyButton.disabled = !result;
    elements.timingLine.textContent = "이 기기에서 번역 완료 · 원문/결과 전송 없음";
    elements.performanceStatus.textContent =
      `최근 모델 열기 ${formatMilliseconds(state.loadDurationMs)} · ` +
      `최근 번역 ${formatMilliseconds(generationDuration)}`;
  } catch (error) {
    console.error(error);
    elements.translatedText.setAttribute("aria-busy", "false");
    elements.translatedText.textContent = `번역 실패: ${error.message}`;
    elements.timingLine.textContent = "민감한 원문을 피드백에 붙이지 말고 실행 환경만 알려 주세요.";
  } finally {
    state.busy = false;
    state.translating = false;
    lockTranslationControls(false);
    refreshTranslateButton();
  }
}

async function resumePendingTranslation() {
  if (!state.pendingTranslation || !state.activeFile || state.busy) return;
  state.pendingTranslation = false;
  await translate();
}

function swapDirection() {
  const source = elements.sourceLanguage.value;
  const sourceText = elements.sourceText.value;
  const translatedText = elements.copyButton.disabled ? "" : elements.translatedText.textContent;
  elements.sourceLanguage.value = elements.targetLanguage.value;
  elements.targetLanguage.value = source;
  if (translatedText) {
    elements.sourceText.value = translatedText;
    elements.translatedText.textContent = sourceText;
    elements.copyButton.disabled = !sourceText.trim();
    elements.timingLine.textContent = "언어와 내용을 서로 바꿨습니다.";
    elements.characterCount.textContent =
      `${elements.sourceText.value.length.toLocaleString("ko-KR")} / 6,000`;
    refreshTranslateButton();
  }
}

async function checkCompatibility() {
  const brands = navigator.userAgentData?.brands ?? [];
  const hasGoogleChromeBrand = brands.some(({ brand }) => brand === "Google Chrome");
  const isBrave = Boolean(navigator.brave);
  const isChrome =
    !isBrave &&
    !/Edg\/|OPR\//.test(navigator.userAgent) &&
    (hasGoogleChromeBrand || /Chrome\//.test(navigator.userAgent));
  const secureEnough = window.isSecureContext;
  const webGpuAdapter = isChrome && navigator.gpu
    ? await navigator.gpu.requestAdapter().catch(() => null)
    : null;
  const hasWebGpu = Boolean(webGpuAdapter);
  const hasOpfs = Boolean(navigator.storage?.getDirectory);
  state.compatible = isChrome && secureEnough && hasWebGpu && hasOpfs;

  const checks = [
    ["Chrome", isChrome],
    ["HTTPS", secureEnough],
    ["WebGPU", hasWebGpu],
    ["브라우저 파일 저장", hasOpfs],
    [crossOriginIsolated ? "멀티스레드 격리" : "단일 스레드 모드", true],
  ];
  const summary = checks.map(([label, ok]) => `${ok ? "✓" : "✕"} ${label}`).join(" · ");
  elements.compatibility.textContent = state.compatible
    ? crossOriginIsolated
      ? `${summary} · 이 기기에서 알파를 실행할 수 있습니다.`
      : `${summary} · CPU 전처리는 단일 스레드, 모델 추론은 WebGPU로 실행합니다.`
    : `${summary} · 최신 데스크톱 Chrome과 HTTPS 환경이 필요합니다.`;
  elements.compatibility.dataset.state = state.compatible ? "ready" : "error";

  if (state.compatible) {
    elements.environmentNotice.hidden = true;
  } else {
    const message = !isChrome
      ? isBrave
        ? "Brave가 아니라 최신 데스크톱 Chrome에서 열어 주세요."
        : "이 알파는 최신 데스크톱 Chrome에서 사용할 수 있습니다."
      : !secureEnough
        ? "안전한 HTTPS 주소에서 다시 열어 주세요."
        : !hasWebGpu
          ? "이 Chrome 환경에서 WebGPU를 사용할 수 없습니다. 그래픽 가속 상태를 확인해 주세요."
          : "이 Chrome 환경에서 브라우저 모델 저장소를 사용할 수 없습니다.";
    elements.environmentNotice.textContent = message;
    elements.environmentNotice.hidden = false;
    setBadge(elements.engineBadge, "환경 확인 필요", "error");
  }
  refreshTranslateButton();
}

elements.installButton.addEventListener("click", downloadModel);
elements.pauseButton.addEventListener("click", () => state.downloadController?.abort());
elements.modelFileInput.addEventListener("change", (event) => useExistingFile(event.target.files?.[0]));
elements.exportButton.addEventListener("click", exportCachedModel);
elements.translateButton.addEventListener("click", translate);
elements.swapButton.addEventListener("click", swapDirection);
elements.sourceLanguage.addEventListener("change", () => {
  elements.targetLanguage.value = elements.sourceLanguage.value === "ko" ? "en" : "ko";
  if (!elements.copyButton.disabled) resetResult("언어가 바뀌었습니다. 다시 번역해 주세요.");
});
elements.targetLanguage.addEventListener("change", () => {
  elements.sourceLanguage.value = elements.targetLanguage.value === "ko" ? "en" : "ko";
  if (!elements.copyButton.disabled) resetResult("언어가 바뀌었습니다. 다시 번역해 주세요.");
});
elements.sourceText.addEventListener("input", () => {
  if (!elements.copyButton.disabled && !state.translating) {
    resetResult("원문이 바뀌었습니다. 다시 번역해 주세요.");
  }
  elements.characterCount.textContent =
    `${elements.sourceText.value.length.toLocaleString("ko-KR")} / 6,000`;
  refreshTranslateButton();
});
elements.sourceText.addEventListener("keydown", (event) => {
  if ((event.ctrlKey || event.metaKey) && event.key === "Enter") {
    event.preventDefault();
    translate();
  }
});
elements.clearButton.addEventListener("click", () => {
  elements.sourceText.value = "";
  elements.characterCount.textContent = "0 / 6,000";
  resetResult();
  state.pendingTranslation = false;
  refreshTranslateButton();
  elements.sourceText.focus();
});
elements.copyButton.addEventListener("click", async () => {
  try {
    await navigator.clipboard.writeText(elements.translatedText.textContent);
    elements.copyButton.textContent = "복사됨";
  } catch {
    elements.copyButton.textContent = "복사 실패";
  }
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
    elements.feedbackLink.href = APP_CONFIG.links.feedback;
    elements.repositoryLink.href = APP_CONFIG.links.repository;
    elements.modelRepository.textContent = MODEL.repository;
    elements.modelRevision.textContent = MODEL.revision;
    elements.modelBytes.textContent = `${MODEL.bytes.toLocaleString("en-US")} bytes`;
    elements.modelSha256.textContent = MODEL.sha256;
    elements.modelLicense.textContent = MODEL.license;
    elements.downloadProgress.max = MODEL.bytes;
    elements.modelSetup.dataset.productGoal = PRODUCT_GOAL_MODEL.id;
    refreshTranslateButton();
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
      elements.performanceStatus.textContent =
        `마지막 로컬 모델 열기 ${formatMilliseconds(lastLoad.durationMs)} · ` +
        `${new Date(lastLoad.measuredAt).toLocaleString("ko-KR")}`;
    }
    if (
      !bootInteractionDetected &&
      document.activeElement === document.body &&
      window.matchMedia("(pointer: fine)").matches
    ) {
      elements.sourceText.focus({ preventScroll: true });
    }
  } catch (error) {
    console.error(error);
    setBadge(elements.modelBadge, "확인 실패", "error");
    elements.compatibility.textContent = `브라우저 저장소 확인 실패: ${error.message}`;
  }
}

boot();
