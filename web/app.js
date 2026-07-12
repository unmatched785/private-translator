const LANGUAGES = [
  ["auto", "언어 자동 감지"],
  ["ko", "한국어"],
  ["en", "영어"],
  ["ja", "일본어"],
  ["zh", "중국어 간체"],
  ["zh-Hant", "중국어 번체"],
  ["fr", "프랑스어"],
  ["de", "독일어"],
  ["es", "스페인어"],
  ["pt", "포르투갈어"],
  ["it", "이탈리아어"],
  ["ru", "러시아어"],
  ["ar", "아랍어"],
  ["tr", "튀르키예어"],
  ["th", "태국어"],
  ["vi", "베트남어"],
  ["id", "인도네시아어"],
  ["ms", "말레이어"],
  ["tl", "필리핀어"],
  ["hi", "힌디어"],
  ["pl", "폴란드어"],
  ["cs", "체코어"],
  ["nl", "네덜란드어"],
  ["uk", "우크라이나어"],
  ["he", "히브리어"],
  ["fa", "페르시아어"],
  ["bn", "벵골어"],
  ["ta", "타밀어"],
  ["te", "텔루구어"],
  ["mr", "마라티어"],
  ["gu", "구자라트어"],
  ["ur", "우르두어"],
  ["km", "크메르어"],
  ["my", "미얀마어"],
  ["bo", "티베트어"],
  ["kk", "카자흐어"],
  ["mn", "몽골어"],
  ["ug", "위구르어"],
  ["yue", "광둥어"],
];

const LANGUAGE_NAMES = Object.fromEntries(LANGUAGES);

const refs = {
  profileBadge: document.querySelector("#profileBadge"),
  sourceLanguage: document.querySelector("#sourceLanguage"),
  targetLanguage: document.querySelector("#targetLanguage"),
  swapLanguages: document.querySelector("#swapLanguages"),
  modelSelect: document.querySelector("#modelSelect"),
  modelDescription: document.querySelector("#modelDescription"),
  sourceText: document.querySelector("#sourceText"),
  translatedText: document.querySelector("#translatedText"),
  outputState: document.querySelector("#outputState"),
  characterCount: document.querySelector("#characterCount"),
  translationMeta: document.querySelector("#translationMeta"),
  historyState: document.querySelector("#historyState"),
  saveHistory: document.querySelector("#saveHistory"),
  saveHint: document.querySelector("#saveHint"),
  translateButton: document.querySelector("#translateButton"),
  clearSource: document.querySelector("#clearSource"),
  copyOutput: document.querySelector("#copyOutput"),
  deleteActive: document.querySelector("#deleteActive"),
  qaPanel: document.querySelector("#qaPanel"),
  historyList: document.querySelector("#historyList"),
  historySearch: document.querySelector("#historySearch"),
  historyPanel: document.querySelector("#historyPanel"),
  historyToggle: document.querySelector("#historyToggle"),
  historyClose: document.querySelector("#historyClose"),
  mobileScrim: document.querySelector("#mobileScrim"),
  toast: document.querySelector("#toast"),
};

const state = {
  config: null,
  records: [],
  activeRecordId: null,
  favoritesOnly: false,
  translating: false,
  requestController: null,
  searchTimer: null,
  toastTimer: null,
};

document.addEventListener("DOMContentLoaded", initialize);

async function initialize() {
  populateLanguages();
  bindEvents();

  try {
    const [config, health] = await Promise.all([api("/api/config"), api("/api/health")]);
    state.config = config;
    refs.profileBadge.textContent = `${config.profile.toUpperCase()} · 기록 ${health.history_count}개`;
    populateModels(config);
    restorePreferences();
    updateModelDescription();
    await loadHistory();
  } catch (error) {
    showToast(error.message, true);
    refs.profileBadge.textContent = "연결 오류";
    refs.historyList.replaceChildren(emptyMessage("로컬 기록고를 열 수 없습니다."));
  }
}

function populateLanguages() {
  for (const [code, label] of LANGUAGES) {
    refs.sourceLanguage.append(new Option(label, code));
    if (code !== "auto") {
      refs.targetLanguage.append(new Option(label, code));
    }
  }
  refs.sourceLanguage.value = "auto";
  refs.targetLanguage.value = "ko";
}

function populateModels(config) {
  refs.modelSelect.replaceChildren();
  for (const model of config.models) {
    const suffix = model.privacy === "device" ? " · 이 장치" : " · 개인 네트워크";
    refs.modelSelect.append(new Option(`${model.label}${suffix}`, model.id));
  }
  refs.modelSelect.value = config.default_model;
}

function restorePreferences() {
  const target = localStorage.getItem("translator.target");
  const model = localStorage.getItem(`translator.model.${state.config.profile}`);
  if (target && [...refs.targetLanguage.options].some((option) => option.value === target)) {
    refs.targetLanguage.value = target;
  }
  if (model && [...refs.modelSelect.options].some((option) => option.value === model)) {
    refs.modelSelect.value = model;
  }
}

function bindEvents() {
  refs.sourceText.addEventListener("input", updateCharacterCount);
  refs.sourceText.addEventListener("paste", () => {
    window.setTimeout(() => translateCurrent("paste"), 40);
  });
  refs.sourceText.addEventListener("keydown", (event) => {
    if (event.key === "Enter" && (event.ctrlKey || event.metaKey)) {
      event.preventDefault();
      translateCurrent("shortcut");
    }
  });
  refs.translateButton.addEventListener("click", () => translateCurrent("button"));
  refs.clearSource.addEventListener("click", clearWorkspace);
  refs.copyOutput.addEventListener("click", copyOutput);
  refs.deleteActive.addEventListener("click", deleteActiveRecord);
  refs.swapLanguages.addEventListener("click", swapLanguages);
  refs.modelSelect.addEventListener("change", () => {
    if (state.config) {
      localStorage.setItem(`translator.model.${state.config.profile}`, refs.modelSelect.value);
    }
    updateModelDescription();
  });
  refs.targetLanguage.addEventListener("change", () => {
    localStorage.setItem("translator.target", refs.targetLanguage.value);
  });
  refs.saveHistory.addEventListener("change", updateSaveState);
  refs.historySearch.addEventListener("input", () => {
    window.clearTimeout(state.searchTimer);
    state.searchTimer = window.setTimeout(loadHistory, 220);
  });
  document.querySelectorAll(".filter-chip").forEach((button) => {
    button.addEventListener("click", () => {
      document.querySelectorAll(".filter-chip").forEach((chip) => chip.classList.remove("active"));
      button.classList.add("active");
      state.favoritesOnly = button.dataset.filter === "favorites";
      loadHistory();
    });
  });
  refs.historyToggle.addEventListener("click", openHistoryPanel);
  refs.historyClose.addEventListener("click", closeHistoryPanel);
  refs.mobileScrim.addEventListener("click", closeHistoryPanel);
}

async function translateCurrent(trigger) {
  if (state.translating && trigger === "paste") {
    state.requestController?.abort();
  }

  const text = refs.sourceText.value;
  if (!text.trim()) {
    showToast("번역할 내용을 입력하세요.", true);
    refs.sourceText.focus();
    return;
  }
  if (!state.config) {
    showToast("로컬 번역 서비스가 아직 준비되지 않았습니다.", true);
    return;
  }

  state.requestController?.abort();
  const controller = new AbortController();
  state.requestController = controller;
  state.translating = true;
  setLoadingState(true);

  try {
    const result = await api("/api/translate", {
      method: "POST",
      signal: controller.signal,
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({
        text,
        source: refs.sourceLanguage.value,
        target: refs.targetLanguage.value,
        model: refs.modelSelect.value,
        mode: state.config.profile === "quality" ? "quality" : "instant",
        save_history: refs.saveHistory.checked,
      }),
    });

    showTranslation(result.translated_text);
    const chunkMeta = result.chunk_count > 1 ? ` · ${result.chunk_count}개 구간` : "";
    refs.translationMeta.textContent = `${result.model_label} · ${formatLatency(result.latency_ms)}${chunkMeta}`;
    refs.historyState.textContent = result.history_id ? "암호화 기록 저장됨" : "이번 번역은 기록하지 않음";
    state.activeRecordId = result.history_id;
    refs.deleteActive.hidden = !result.history_id;
    renderQa(result.qa_warnings);
    if (result.history_id) {
      await loadHistory();
    }
  } catch (error) {
    if (error.name !== "AbortError") {
      setEmptyOutput("번역 엔진에 연결하지 못했습니다", error.message);
      showToast(error.message, true);
    }
  } finally {
    if (state.requestController === controller) {
      state.translating = false;
      setLoadingState(false);
    }
  }
}

function setLoadingState(loading) {
  refs.translateButton.disabled = loading;
  refs.translateButton.querySelector(".button-label").textContent = loading ? "번역 중…" : "번역하기";
  if (loading) {
    refs.translatedText.classList.remove("visible");
    refs.outputState.classList.remove("hidden", "empty");
    refs.outputState.classList.add("loading");
    refs.outputState.querySelector("strong").textContent = "로컬 모델이 번역하고 있습니다";
    refs.outputState.querySelector(":scope > span").textContent = "텍스트는 선택한 개인 장치 밖으로 전송되지 않습니다.";
    refs.translationMeta.textContent = "처리 중";
  }
}

function showTranslation(text) {
  refs.translatedText.value = text;
  refs.outputState.classList.add("hidden");
  refs.outputState.classList.remove("loading");
  refs.translatedText.classList.add("visible");
}

function setEmptyOutput(title, message) {
  refs.translatedText.value = "";
  refs.translatedText.classList.remove("visible");
  refs.outputState.classList.remove("hidden", "loading");
  refs.outputState.classList.add("empty");
  refs.outputState.querySelector("strong").textContent = title;
  refs.outputState.querySelector(":scope > span").textContent = message;
  refs.translationMeta.textContent = "준비됨";
}

function renderQa(warnings = []) {
  refs.qaPanel.replaceChildren();
  if (!warnings.length) {
    refs.qaPanel.hidden = true;
    return;
  }
  const strong = document.createElement("strong");
  strong.textContent = "확인 권장: ";
  refs.qaPanel.append(strong, document.createTextNode(warnings.join(" · ")));
  refs.qaPanel.hidden = false;
}

async function loadHistory() {
  const params = new URLSearchParams({ limit: "120" });
  const query = refs.historySearch.value.trim();
  if (query) params.set("q", query);
  if (state.favoritesOnly) params.set("favorites", "true");

  try {
    const response = await api(`/api/history?${params}`);
    state.records = response.records;
    renderHistory();
    if (state.config) {
      refs.profileBadge.textContent = `${state.config.profile.toUpperCase()} · 기록 ${state.records.length}개`;
    }
  } catch (error) {
    refs.historyList.replaceChildren(emptyMessage("기록을 불러올 수 없습니다."));
    showToast(error.message, true);
  }
}

function renderHistory() {
  refs.historyList.replaceChildren();
  if (!state.records.length) {
    const message = refs.historySearch.value.trim()
      ? "검색 결과가 없습니다.\n기록은 암호화된 상태로 로컬에 남아 있습니다."
      : state.favoritesOnly
        ? "즐겨찾기한 번역이 없습니다."
        : "아직 저장된 번역이 없습니다.\n첫 번역을 붙여넣어 시작하세요.";
    refs.historyList.append(emptyMessage(message));
    return;
  }

  let previousDay = "";
  for (const record of state.records) {
    const day = formatDay(record.created_at);
    if (day !== previousDay) {
      const heading = document.createElement("div");
      heading.className = "history-day";
      heading.textContent = day;
      refs.historyList.append(heading);
      previousDay = day;
    }

    const item = document.createElement("article");
    item.className = `history-item${record.id === state.activeRecordId ? " active" : ""}`;
    item.dataset.id = record.id;

    const openButton = document.createElement("button");
    openButton.className = "history-open";
    openButton.type = "button";
    openButton.setAttribute("aria-label", `${record.source_text.slice(0, 50)} 번역 기록 열기`);

    const source = document.createElement("p");
    source.className = "history-item-source";
    source.textContent = record.source_text.replace(/\s+/g, " ");

    const output = document.createElement("p");
    output.className = "history-item-output";
    output.textContent = record.translated_text.replace(/\s+/g, " ");

    const meta = document.createElement("div");
    meta.className = "history-item-meta";
    meta.append(
      document.createTextNode(`${languageLabel(record.source_lang)} → ${languageLabel(record.target_lang)}`),
      document.createTextNode("·"),
      document.createTextNode(formatTime(record.created_at)),
    );

    const star = document.createElement("button");
    star.className = `history-star${record.favorite ? " on" : ""}`;
    star.type = "button";
    star.textContent = record.favorite ? "★" : "☆";
    star.setAttribute("aria-label", record.favorite ? "즐겨찾기 해제" : "즐겨찾기 추가");
    star.addEventListener("click", (event) => {
      event.stopPropagation();
      toggleFavorite(record);
    });

    openButton.append(source, output, meta);
    openButton.addEventListener("click", () => openHistoryRecord(record));
    item.append(openButton, star);
    refs.historyList.append(item);
  }
}

function openHistoryRecord(record) {
  state.activeRecordId = record.id;
  refs.sourceText.value = record.source_text;
  if ([...refs.sourceLanguage.options].some((option) => option.value === record.source_lang)) {
    refs.sourceLanguage.value = record.source_lang;
  }
  if ([...refs.targetLanguage.options].some((option) => option.value === record.target_lang)) {
    refs.targetLanguage.value = record.target_lang;
  }
  if ([...refs.modelSelect.options].some((option) => option.value === record.model_id)) {
    refs.modelSelect.value = record.model_id;
  }
  showTranslation(record.translated_text);
  refs.translationMeta.textContent = `${record.model_label} · ${formatLatency(record.latency_ms)}`;
  refs.historyState.textContent = "암호화 기록에서 열림";
  refs.deleteActive.hidden = false;
  renderQa(record.qa_warnings);
  updateCharacterCount();
  updateModelDescription();
  renderHistory();
  closeHistoryPanel();
}

async function toggleFavorite(record) {
  try {
    await api(`/api/history/${encodeURIComponent(record.id)}/favorite`, {
      method: "PATCH",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ favorite: !record.favorite }),
    });
    record.favorite = !record.favorite;
    renderHistory();
  } catch (error) {
    showToast(error.message, true);
  }
}

async function deleteActiveRecord() {
  if (!state.activeRecordId) return;
  if (!window.confirm("이 번역 기록을 로컬 기록고에서 삭제할까요?")) return;

  try {
    await api(`/api/history/${encodeURIComponent(state.activeRecordId)}`, { method: "DELETE" });
    state.activeRecordId = null;
    refs.deleteActive.hidden = true;
    refs.historyState.textContent = "기록 삭제됨";
    await loadHistory();
    showToast("번역 기록을 삭제했습니다.");
  } catch (error) {
    showToast(error.message, true);
  }
}

function updateCharacterCount() {
  const count = [...refs.sourceText.value].length;
  const max = state.config?.max_text_chars;
  refs.characterCount.textContent = max ? `${count.toLocaleString()} / ${max.toLocaleString()}자` : `${count.toLocaleString()}자`;
}

function updateSaveState() {
  const saving = refs.saveHistory.checked;
  refs.saveHint.textContent = saving ? "암호화하여 로컬에 보관" : "이번 번역은 저장하지 않음";
  refs.historyState.textContent = saving ? "기록 저장 켜짐" : "비공개 번역 모드";
}

function updateModelDescription() {
  const model = state.config?.models.find((candidate) => candidate.id === refs.modelSelect.value);
  if (!model) return;
  const privacy = model.privacy === "device" ? "이 장치에서 처리" : "개인 네트워크 장치에서 처리";
  refs.modelDescription.textContent = `${model.description} · ${privacy}`;
}

function swapLanguages() {
  const source = refs.sourceLanguage.value;
  const target = refs.targetLanguage.value;
  refs.sourceLanguage.value = source === "auto" ? target : target;
  refs.targetLanguage.value = source === "auto" ? (target === "ko" ? "en" : "ko") : source;
  const sourceText = refs.sourceText.value;
  const translatedText = refs.translatedText.value;
  if (translatedText.trim()) {
    refs.sourceText.value = translatedText;
    showTranslation(sourceText);
  }
  updateCharacterCount();
}

function clearWorkspace() {
  refs.sourceText.value = "";
  state.activeRecordId = null;
  refs.deleteActive.hidden = true;
  setEmptyOutput("번역 결과가 여기에 표시됩니다", "모든 처리는 선택한 개인 장치 안에서 이루어집니다.");
  refs.historyState.textContent = refs.saveHistory.checked ? "기록 저장 켜짐" : "비공개 번역 모드";
  renderQa([]);
  updateCharacterCount();
  refs.sourceText.focus();
  renderHistory();
}

async function copyOutput() {
  const text = refs.translatedText.value;
  if (!text) {
    showToast("복사할 번역문이 없습니다.", true);
    return;
  }
  try {
    await navigator.clipboard.writeText(text);
    showToast("번역문을 복사했습니다.");
  } catch {
    refs.translatedText.select();
    document.execCommand("copy");
    showToast("번역문을 복사했습니다.");
  }
}

function openHistoryPanel() {
  refs.historyPanel.classList.add("open");
  refs.mobileScrim.classList.add("show");
}

function closeHistoryPanel() {
  refs.historyPanel.classList.remove("open");
  refs.mobileScrim.classList.remove("show");
}

function emptyMessage(message) {
  const element = document.createElement("div");
  element.className = "history-empty";
  element.textContent = message;
  return element;
}

function languageLabel(code) {
  return LANGUAGE_NAMES[code] || code;
}

function formatDay(timestamp) {
  const date = new Date(timestamp);
  const today = new Date();
  const yesterday = new Date();
  yesterday.setDate(today.getDate() - 1);
  if (date.toDateString() === today.toDateString()) return "오늘";
  if (date.toDateString() === yesterday.toDateString()) return "어제";
  return new Intl.DateTimeFormat("ko-KR", { month: "long", day: "numeric" }).format(date);
}

function formatTime(timestamp) {
  return new Intl.DateTimeFormat("ko-KR", { hour: "2-digit", minute: "2-digit" }).format(new Date(timestamp));
}

function formatLatency(milliseconds) {
  if (milliseconds < 1000) return `${milliseconds}ms`;
  return `${(milliseconds / 1000).toFixed(1)}초`;
}

function showToast(message, error = false) {
  window.clearTimeout(state.toastTimer);
  refs.toast.textContent = message;
  refs.toast.classList.toggle("error", error);
  refs.toast.classList.add("show");
  state.toastTimer = window.setTimeout(() => refs.toast.classList.remove("show"), 3200);
}

async function api(path, options = {}) {
  const response = await fetch(path, {
    credentials: "same-origin",
    cache: "no-store",
    ...options,
  });
  if (response.status === 204) return null;
  const contentType = response.headers.get("content-type") || "";
  const body = contentType.includes("application/json") ? await response.json() : null;
  if (!response.ok) {
    throw new Error(body?.error?.message || `요청을 처리할 수 없습니다 (${response.status})`);
  }
  return body;
}
