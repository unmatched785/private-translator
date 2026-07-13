const LANGUAGE_CODES = [
  "ko",
  "en",
  "ja",
  "zh",
  "zh-Hant",
  "fr",
  "de",
  "es",
  "pt",
  "it",
  "ru",
  "ar",
  "tr",
  "th",
  "vi",
  "id",
  "ms",
  "tl",
  "hi",
  "pl",
  "cs",
  "nl",
  "uk",
  "he",
  "fa",
  "bn",
  "ta",
  "te",
  "mr",
  "gu",
  "ur",
  "km",
  "my",
  "bo",
  "kk",
  "mn",
  "ug",
  "yue",
];

const LANGUAGE_FALLBACK_NAMES = {
  ko: "Korean",
  en: "English",
  ja: "Japanese",
  zh: "Chinese (Simplified)",
  "zh-Hant": "Chinese (Traditional)",
  fr: "French",
  de: "German",
  es: "Spanish",
  pt: "Portuguese",
  it: "Italian",
  ru: "Russian",
  ar: "Arabic",
  tr: "Turkish",
  th: "Thai",
  vi: "Vietnamese",
  id: "Indonesian",
  ms: "Malay",
  tl: "Filipino",
  hi: "Hindi",
  pl: "Polish",
  cs: "Czech",
  nl: "Dutch",
  uk: "Ukrainian",
  he: "Hebrew",
  fa: "Persian",
  bn: "Bengali",
  ta: "Tamil",
  te: "Telugu",
  mr: "Marathi",
  gu: "Gujarati",
  ur: "Urdu",
  km: "Khmer",
  my: "Burmese",
  bo: "Tibetan",
  kk: "Kazakh",
  mn: "Mongolian",
  ug: "Uyghur",
  yue: "Cantonese",
};

const LANGUAGE_LOCALE_OVERRIDES = {
  ko: {
    bo: "티베트어",
  },
};

const SESSION_TOKEN_STORAGE_KEY = "private-translator.session-token";
const SESSION_TOKEN_PATTERN = /^[0-9a-f]{64}$/i;

const i18n = globalThis.TranslatorI18n;
const t = (key, variables = {}, fallback) => i18n.t(key, variables, fallback);
let languageDisplayNames = createLanguageDisplayNames();

const refs = {
  uiLocale: document.querySelector("#uiLocale"),
  profileBadge: document.querySelector("#profileBadge"),
  privacyBadge: document.querySelector("#privacyBadge"),
  privacyBadgeText: document.querySelector("#privacyBadgeText"),
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
  memoryAction: document.querySelector("#memoryAction"),
  cancelMemoryEdit: document.querySelector("#cancelMemoryEdit"),
  deleteMemory: document.querySelector("#deleteMemory"),
  deleteActive: document.querySelector("#deleteActive"),
  qaPanel: document.querySelector("#qaPanel"),
  historyList: document.querySelector("#historyList"),
  historySearch: document.querySelector("#historySearch"),
  historyPanel: document.querySelector("#historyPanel"),
  historyToggle: document.querySelector("#historyToggle"),
  historyClose: document.querySelector("#historyClose"),
  mobileScrim: document.querySelector("#mobileScrim"),
  toast: document.querySelector("#toast"),
  translationAnnouncer: document.querySelector("#translationAnnouncer"),
  topbar: document.querySelector(".topbar"),
  workspace: document.querySelector(".translator-workspace"),
};

const historyDrawerMedia = window.matchMedia("(max-width: 760px)");

const state = {
  sessionToken: null,
  config: null,
  records: [],
  activeRecordId: null,
  activeMemoryId: null,
  activeMemoryRevision: null,
  editingMemory: false,
  memoryEditSnapshot: null,
  historyFilter: "all",
  translating: false,
  requestController: null,
  clientId: createClientId(),
  requestSequence: 0,
  searchTimer: null,
  toastTimer: null,
  previousModelId: null,
  privateNetworkConsent: new Set(),
  historyReturnFocus: null,
  qaWarnings: [],
  translationRestoreView: null,
  displayedResultContext: null,
  historyMessage: { key: "history.save_on", variables: {} },
  translationMeta: { kind: "ready" },
};

document.addEventListener("DOMContentLoaded", initialize);

async function initialize() {
  i18n.applyDocument();
  refs.uiLocale.value = i18n.getLocale();
  populateLanguages();
  bindEvents();
  syncHistoryPanelAccessibility();

  try {
    await bootstrapSession();
    const [config, health] = await Promise.all([api("/api/config"), api("/api/health")]);
    state.config = config;
    populateModels(config);
    restorePreferences();
    populateLanguages(refs.sourceLanguage.value, refs.targetLanguage.value);
    updateModelDescription();
    updateCharacterCount();
    updateProfileBadge(health.history_count);
    await loadHistory();
  } catch (error) {
    showToast(error.message, true);
    refs.profileBadge.textContent = t("status.connection_error");
    refs.historyList.replaceChildren(emptyMessage(t("error.vault_open")));
  }
}

async function bootstrapSession() {
  const fragment = new URLSearchParams(window.location.hash.slice(1));
  const fragmentToken = fragment.get("token");
  if (fragmentToken !== null) {
    window.history.replaceState(null, "", `${window.location.pathname}${window.location.search}`);
    if (SESSION_TOKEN_PATTERN.test(fragmentToken)) {
      state.sessionToken = fragmentToken;
      try {
        window.sessionStorage.setItem(SESSION_TOKEN_STORAGE_KEY, fragmentToken);
      } catch {
        // The in-memory token still authorizes this tab when storage is disabled.
      }
    }
  } else {
    try {
      const storedToken = window.sessionStorage.getItem(SESSION_TOKEN_STORAGE_KEY);
      if (storedToken && SESSION_TOKEN_PATTERN.test(storedToken)) state.sessionToken = storedToken;
    } catch {
      // A fresh launch URL can still authorize the tab when storage is disabled.
    }
  }
  if (!state.sessionToken) {
    throw new Error(t("error.session_required"));
  }
}

function createLanguageDisplayNames() {
  try {
    return new Intl.DisplayNames([i18n.getLocale()], { type: "language" });
  } catch {
    return null;
  }
}

function localizedLanguageName(code) {
  if (code === "auto") return t("language.auto");
  const displayCode = code === "zh" ? "zh-Hans" : code;
  try {
    const localized = languageDisplayNames?.of(displayCode);
    return localized && localized !== displayCode && localized !== code
      ? localized
      : LANGUAGE_LOCALE_OVERRIDES[i18n.getLocale()]?.[code] || LANGUAGE_FALLBACK_NAMES[code] || code;
  } catch {
    return LANGUAGE_LOCALE_OVERRIDES[i18n.getLocale()]?.[code] || LANGUAGE_FALLBACK_NAMES[code] || code;
  }
}

function localizedModelLabel(modelId, fallback = modelId) {
  return t(`model.${modelId}.label`, {}, fallback);
}

function localizedModelDescription(model) {
  return t(`model.${model.id}.description`, {}, model.description);
}

function localizedPrivacy(privacy) {
  return privacy === "device" ? t("privacy.device") : t("privacy.private_network");
}

function selectedModel() {
  return state.config?.models.find((candidate) => candidate.id === refs.modelSelect.value) || null;
}

function modelIsAvailable(model) {
  return Boolean(model) && model.available !== false;
}

function populateLanguages(source = refs.sourceLanguage.value || "auto", target = refs.targetLanguage.value || "ko") {
  refs.sourceLanguage.replaceChildren();
  refs.targetLanguage.replaceChildren();
  refs.sourceLanguage.append(new Option(localizedLanguageName("auto"), "auto"));
  for (const code of supportedLanguageCodes()) {
    const label = localizedLanguageName(code);
    refs.sourceLanguage.append(new Option(label, code));
    refs.targetLanguage.append(new Option(label, code));
  }
  refs.sourceLanguage.value = [...refs.sourceLanguage.options].some((option) => option.value === source)
    ? source
    : "auto";
  refs.targetLanguage.value = [...refs.targetLanguage.options].some((option) => option.value === target)
    ? target
    : "ko";
}

function supportedLanguageCodes() {
  if (!state.config) return LANGUAGE_CODES;
  const model = selectedModel();
  return Array.isArray(model?.supported_languages) ? model.supported_languages : LANGUAGE_CODES;
}

function populateModels(config, selected = refs.modelSelect.value || config.default_model) {
  refs.modelSelect.replaceChildren();
  for (const model of config.models) {
    const availability = modelIsAvailable(model) ? "" : ` · ${t("model.setup_required")}`;
    const option = new Option(
      `${localizedModelLabel(model.id, model.label)} · ${localizedPrivacy(model.privacy)}${availability}`,
      model.id,
    );
    option.disabled = !modelIsAvailable(model);
    refs.modelSelect.append(option);
  }
  const preferred = config.models.find((model) => model.id === selected && modelIsAvailable(model));
  const configuredDefault = config.models.find(
    (model) => model.id === config.default_model && modelIsAvailable(model),
  );
  const fallback = config.models.find(modelIsAvailable) || config.models[0];
  refs.modelSelect.value = (preferred || configuredDefault || fallback)?.id || "";
  state.previousModelId = refs.modelSelect.value;
  updateTranslateAvailability();
}

function restorePreferences() {
  const target = localStorage.getItem("translator.target");
  const model = localStorage.getItem(`translator.model.${state.config.profile}`);
  if (target && [...refs.targetLanguage.options].some((option) => option.value === target)) {
    refs.targetLanguage.value = target;
  }
  if (model && [...refs.modelSelect.options].some((option) => option.value === model && !option.disabled)) {
    refs.modelSelect.value = model;
    state.previousModelId = model;
  }
}

function bindEvents() {
  refs.uiLocale.addEventListener("change", () => i18n.setLocale(refs.uiLocale.value));
  window.addEventListener("translator:locale-change", refreshLocale);
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
  refs.memoryAction.addEventListener("click", handleMemoryAction);
  refs.cancelMemoryEdit.addEventListener("click", cancelMemoryEdit);
  refs.deleteMemory.addEventListener("click", deleteActiveMemory);
  refs.deleteActive.addEventListener("click", deleteActiveRecord);
  refs.swapLanguages.addEventListener("click", swapLanguages);
  refs.modelSelect.addEventListener("change", handleModelChange);
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
      state.historyFilter = button.dataset.filter;
      loadHistory();
    });
  });
  refs.historyToggle.addEventListener("click", openHistoryPanel);
  refs.historyClose.addEventListener("click", closeHistoryPanel);
  refs.mobileScrim.addEventListener("click", closeHistoryPanel);
  document.addEventListener("keydown", handleHistoryPanelKeydown);
  window.addEventListener("beforeunload", handleBeforeUnload);
  historyDrawerMedia.addEventListener("change", syncHistoryPanelAccessibility);
}

function refreshLocale() {
  const source = refs.sourceLanguage.value;
  const target = refs.targetLanguage.value;
  const model = refs.modelSelect.value;
  languageDisplayNames = createLanguageDisplayNames();
  i18n.applyDocument();
  refs.uiLocale.value = i18n.getLocale();
  populateLanguages(source, target);
  if (state.config) populateModels(state.config, model);
  updateCharacterCount();
  updateModelDescription();
  updateSaveState(false);
  updateMemoryControls();
  updateProfileBadge();
  renderHistory();
  renderHistoryState();
  renderTranslationMeta();
  syncHistoryPanelAccessibility();
}

function updateProfileBadge(count = state.records.length) {
  if (!state.config) return;
  const key = state.historyFilter === "approved" ? "profile.assets" : "profile.records";
  refs.profileBadge.textContent = t(key, {
    profile: state.config.profile.toUpperCase(),
    count: formatNumber(count),
  });
}

function setHistoryState(key, variables = {}) {
  state.historyMessage = { key, variables };
  renderHistoryState();
}

function renderHistoryState() {
  refs.historyState.textContent = t(state.historyMessage.key, state.historyMessage.variables);
}

function setTranslationMeta(meta) {
  state.translationMeta = meta;
  renderTranslationMeta();
}

function renderTranslationMeta() {
  const meta = state.translationMeta;
  if (meta.kind === "preserved") {
    const previous = meta.previous;
    const model = previous.modelId
      ? localizedModelLabel(previous.modelId, previous.modelLabel)
      : t("output.ready");
    refs.translationMeta.textContent = `${t("output.previous_preserved")} · ${model}`;
    return;
  }
  if (meta.kind === "processing") {
    refs.translationMeta.textContent = t("output.processing");
    return;
  }
  if (meta.kind === "result") {
    const chunks = meta.chunkCount > 1
      ? ` · ${t("output.chunk_count", { count: formatNumber(meta.chunkCount) })}`
      : "";
    const asset = meta.memoryRevision
      ? ` · ${t("memory.meta", { revision: meta.memoryRevision })}`
      : "";
    refs.translationMeta.textContent = `${localizedModelLabel(meta.modelId, meta.modelLabel)} · ${formatLatency(meta.latencyMs)}${chunks}${asset}`;
    return;
  }
  if (meta.kind === "asset") {
    refs.translationMeta.textContent = `${localizedModelLabel(meta.modelId, meta.modelLabel)} · ${t("memory.meta", { revision: meta.revision })}`;
    return;
  }
  refs.translationMeta.textContent = t("output.ready");
}

async function translateCurrent(trigger) {
  if (state.translating && trigger === "paste") {
    state.requestController?.abort();
  }

  const text = refs.sourceText.value;
  if (!text.trim()) {
    showToast(t("translate.empty"), true);
    refs.sourceText.focus();
    return;
  }
  if (!state.config) {
    showToast(t("translate.not_ready"), true);
    return;
  }
  if ([...text].length > state.config.max_text_chars) {
    showToast(t("translate.too_long", { max: formatNumber(state.config.max_text_chars) }), true);
    return;
  }

  const model = selectedModel();
  if (!modelIsAvailable(model)) {
    showToast(t("translate.model_unavailable"), true);
    return;
  }
  if (!ensurePrivateNetworkConsent(model)) return;
  if (!confirmDiscardMemoryEdit()) return;
  if (!confirmDiscardUnpersistedResult()) return;

  state.translationRestoreView = captureTranslationRestoreView() || state.translationRestoreView;
  state.activeRecordId = null;
  state.activeMemoryId = null;
  state.activeMemoryRevision = null;
  refs.deleteActive.hidden = true;
  updateMemoryControls();
  state.requestController?.abort();
  const controller = new AbortController();
  const requestSequence = ++state.requestSequence;
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
        client_id: state.clientId,
        request_seq: requestSequence,
      }),
    });

    if (requestSequence !== state.requestSequence) return;
    showTranslation(result.translated_text);
    setTranslationMeta({
      kind: "result",
      modelId: result.model_id,
      modelLabel: result.model_label,
      latencyMs: result.latency_ms,
      chunkCount: result.chunk_count,
    });
    const historyStorageFailed = result.history_error === "storage_error";
    setHistoryState(
      historyStorageFailed ? "history.storage_error" : result.history_id ? "history.saved" : "history.not_saved",
    );
    state.activeRecordId = result.history_id;
    refs.deleteActive.hidden = !result.history_id;
    updateMemoryControls();
    renderQa(result.qa_warnings);
    if (result.history_id) {
      await loadHistory();
    }
    if (historyStorageFailed) {
      showToast(t("history.storage_error"), true);
      announceTranslation(t("history.storage_error"));
    }
    state.translationRestoreView = null;
  } catch (error) {
    if (error.name !== "AbortError" && requestSequence === state.requestSequence) {
      if (state.translationRestoreView) {
        restoreTranslationView(state.translationRestoreView);
      } else {
        setEmptyOutput(t("output.engine_failed"), error.message);
      }
      state.translationRestoreView = null;
      showToast(error.message, true);
      announceTranslation(t("output.announce.failed", { message: error.message }));
    }
  } finally {
    if (state.requestController === controller) {
      state.translating = false;
      setLoadingState(false);
    }
  }
}

function createClientId() {
  if (globalThis.crypto?.randomUUID) return globalThis.crypto.randomUUID();
  const random = Math.random().toString(36).slice(2);
  return `browser-${Date.now().toString(36)}-${random}`;
}

function setLoadingState(loading) {
  refs.translateButton.disabled = loading;
  refs.translateButton.querySelector(".button-label").textContent = loading
    ? t("translate.loading")
    : t("translate.button");
  if (loading) {
    refs.translatedText.classList.remove("visible");
    refs.outputState.classList.remove("hidden", "empty");
    refs.outputState.classList.add("loading");
    refs.outputState.querySelector("strong").textContent = t("output.loading_title");
    refs.outputState.querySelector(":scope > span").textContent = t("output.loading_subtitle");
    setTranslationMeta({ kind: "processing" });
    announceTranslation(t("output.announce.loading"));
  } else {
    updateTranslateAvailability();
  }
}

function showTranslation(text, announce = true, context = currentTranslationContext()) {
  refs.translatedText.value = text;
  refs.translatedText.readOnly = true;
  state.displayedResultContext = context ? { ...context } : null;
  refs.outputState.classList.add("hidden");
  refs.outputState.classList.remove("loading");
  refs.translatedText.classList.add("visible");
  if (announce) announceTranslation(t("output.announce.complete"));
}

function captureTranslationRestoreView() {
  if (!refs.translatedText.classList.contains("visible")) return null;
  return {
    translatedText: refs.translatedText.value,
    displayedResultContext: state.displayedResultContext
      ? { ...state.displayedResultContext }
      : null,
    activeRecordId: state.activeRecordId,
    activeMemoryId: state.activeMemoryId,
    activeMemoryRevision: state.activeMemoryRevision,
    historyMessage: {
      key: state.historyMessage.key,
      variables: { ...state.historyMessage.variables },
    },
    translationMeta: { ...state.translationMeta },
    qaWarnings: [...state.qaWarnings],
  };
}

function restoreTranslationView(snapshot) {
  state.activeRecordId = snapshot.activeRecordId;
  state.activeMemoryId = snapshot.activeMemoryId;
  state.activeMemoryRevision = snapshot.activeMemoryRevision;
  const currentContext = currentTranslationContext();
  const contextMatches = translationContextsMatch(currentContext, snapshot.displayedResultContext);
  showTranslation(snapshot.translatedText, false, snapshot.displayedResultContext);
  state.historyMessage = {
    key: snapshot.historyMessage.key,
    variables: { ...snapshot.historyMessage.variables },
  };
  const previousMeta = snapshot.translationMeta.kind === "preserved"
    ? snapshot.translationMeta.previous
    : snapshot.translationMeta;
  state.translationMeta = contextMatches
    ? { ...snapshot.translationMeta }
    : { kind: "preserved", previous: { ...previousMeta } };
  renderHistoryState();
  renderTranslationMeta();
  renderQa(snapshot.qaWarnings);
  refs.deleteActive.hidden = !state.activeRecordId;
  updateMemoryControls();
  renderHistory();
}

function setEmptyOutput(title, message) {
  refs.translatedText.value = "";
  state.displayedResultContext = null;
  refs.translatedText.classList.remove("visible");
  refs.outputState.classList.remove("hidden", "loading");
  refs.outputState.classList.add("empty");
  refs.outputState.querySelector("strong").textContent = title;
  refs.outputState.querySelector(":scope > span").textContent = message;
  setTranslationMeta({ kind: "ready" });
}

function currentTranslationContext() {
  return {
    sourceText: refs.sourceText.value,
    sourceLanguage: refs.sourceLanguage.value,
    targetLanguage: refs.targetLanguage.value,
    modelId: refs.modelSelect.value,
  };
}

function translationContextsMatch(left, right) {
  return Boolean(left && right) &&
    left.sourceText === right.sourceText &&
    left.sourceLanguage === right.sourceLanguage &&
    left.targetLanguage === right.targetLanguage &&
    left.modelId === right.modelId;
}

function renderQa(warnings = []) {
  state.qaWarnings = [...warnings];
  refs.qaPanel.replaceChildren();
  if (!warnings.length) {
    refs.qaPanel.hidden = true;
    return;
  }
  const strong = document.createElement("strong");
  strong.textContent = t("qa.title");
  refs.qaPanel.append(strong, document.createTextNode(warnings.map(localizedQaWarning).join(" · ")));
  refs.qaPanel.hidden = false;
}

function localizedQaWarning(warning) {
  const legacyCodes = {
    "번역 결과가 비어 있습니다": "empty_result",
    "원문의 숫자 또는 단위가 번역문과 다를 수 있습니다": "number_mismatch",
    "URL이 누락되거나 변경되었을 수 있습니다": "url_mismatch",
  };
  const code = legacyCodes[warning] || warning;
  return t(`qa.${code}`, {}, warning);
}

async function loadHistory() {
  const query = refs.historySearch.value.trim();

  try {
    if (state.historyFilter === "approved") {
      const response = await api("/api/memory?limit=500");
      const normalizedQuery = query.toLocaleLowerCase();
      state.records = response.records
        .map(memoryAsHistoryRecord)
        .filter(
          (record) =>
            !normalizedQuery ||
            `${record.source_text}\n${record.translated_text}`.toLocaleLowerCase().includes(normalizedQuery),
        );
    } else {
      const response = await api("/api/history/search", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({
          query,
          favorites: state.historyFilter === "favorites",
          limit: 120,
        }),
      });
      state.records = response.records;
    }
    renderHistory();
    updateProfileBadge();
  } catch (error) {
    refs.historyList.replaceChildren(emptyMessage(t("history.load_failed")));
    showToast(error.message, true);
  }
}

function renderHistory() {
  refs.historyList.replaceChildren();
  if (!state.records.length) {
    const message = refs.historySearch.value.trim()
      ? t("history.empty.search")
      : state.historyFilter === "favorites"
        ? t("history.empty.favorites")
        : state.historyFilter === "approved"
          ? t("history.empty.assets")
          : t("history.empty.all");
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
    const isActive = record.is_memory_asset
      ? record.approved_memory_id === state.activeMemoryId
      : record.id === state.activeRecordId;
    item.className = `history-item${isActive ? " active" : ""}`;
    item.dataset.id = record.id;

    const openButton = document.createElement("button");
    openButton.className = "history-open";
    openButton.type = "button";
    openButton.setAttribute("aria-label", t("history.open_record", { text: record.source_text.slice(0, 50) }));

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
    if (record.approved_revision) {
      meta.append(
        document.createTextNode("·"),
        document.createTextNode(t("memory.meta", { revision: record.approved_revision })),
      );
    }

    const star = document.createElement("button");
    star.className = `history-star${record.favorite ? " on" : ""}`;
    star.type = "button";
    star.textContent = record.favorite ? "★" : "☆";
    star.setAttribute("aria-label", record.favorite ? t("history.favorite.remove") : t("history.favorite.add"));
    star.addEventListener("click", (event) => {
      event.stopPropagation();
      toggleFavorite(record);
    });

    openButton.append(source, output, meta);
    openButton.addEventListener("click", () => openHistoryRecord(record));
    item.append(openButton);
    if (!record.is_memory_asset) item.append(star);
    refs.historyList.append(item);
  }
}

function memoryAsHistoryRecord(memory) {
  return {
    ...memory,
    id: memory.history_id || `memory:${memory.id}`,
    memory_id: memory.id,
    approved_memory_id: memory.id,
    approved_revision: memory.revision,
    created_at: memory.updated_at,
    favorite: false,
    is_memory_asset: true,
  };
}

async function openHistoryRecord(record) {
  if (!confirmDiscardMemoryEdit()) return;
  if (!confirmDiscardUnpersistedResult()) return;
  let openedRecord = record;
  if (record.approved_memory_id && !record.is_memory_asset) {
    try {
      openedRecord = await api(`/api/memory/${encodeURIComponent(record.approved_memory_id)}`);
    } catch (error) {
      showToast(error.message, true);
      return;
    }
  }
  state.activeRecordId = record.is_memory_asset ? record.history_id : record.id;
  state.activeMemoryId = record.approved_memory_id || null;
  state.activeMemoryRevision = openedRecord.revision || record.approved_revision || null;
  refs.sourceText.value = openedRecord.source_text;
  if ([...refs.sourceLanguage.options].some((option) => option.value === openedRecord.source_lang)) {
    refs.sourceLanguage.value = openedRecord.source_lang;
  }
  if ([...refs.targetLanguage.options].some((option) => option.value === openedRecord.target_lang)) {
    refs.targetLanguage.value = openedRecord.target_lang;
  }
  if ([...refs.modelSelect.options].some((option) => option.value === openedRecord.model_id)) {
    refs.modelSelect.value = openedRecord.model_id;
  }
  showTranslation(openedRecord.translated_text, false);
  setTranslationMeta({
    kind: "result",
    modelId: openedRecord.model_id,
    modelLabel: openedRecord.model_label,
    latencyMs: openedRecord.latency_ms,
    chunkCount: openedRecord.chunk_count || 1,
    memoryRevision: state.activeMemoryRevision,
  });
  setHistoryState(state.activeMemoryId ? "memory.opened" : "history.opened");
  refs.deleteActive.hidden = !state.activeRecordId;
  updateMemoryControls();
  renderQa(openedRecord.qa_warnings);
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

async function handleMemoryAction() {
  if (!state.activeMemoryId) {
    if (!state.activeRecordId) return;
    try {
      const memory = await api(`/api/history/${encodeURIComponent(state.activeRecordId)}/approve`, {
        method: "POST",
      });
      state.activeMemoryId = memory.id;
      state.activeMemoryRevision = memory.revision;
      const active = state.records.find((record) => record.id === state.activeRecordId);
      if (active) {
        active.approved_memory_id = memory.id;
        active.approved_revision = memory.revision;
      }
      setHistoryState("memory.approved", { revision: memory.revision });
      updateMemoryControls();
      await loadHistory();
      showToast(t("memory.approve_done"));
    } catch (error) {
      showToast(error.message, true);
    }
    return;
  }

  if (!state.editingMemory) {
    state.editingMemory = true;
    state.memoryEditSnapshot = captureMemoryEditSnapshot();
    refs.translatedText.readOnly = false;
    refs.translatedText.focus();
    refs.memoryAction.textContent = t("memory.save_revision", { revision: state.activeMemoryRevision + 1 });
    refs.cancelMemoryEdit.hidden = false;
    refs.deleteMemory.hidden = true;
    setHistoryState("memory.editing");
    return;
  }

  try {
    const memory = await api(`/api/memory/${encodeURIComponent(state.activeMemoryId)}`, {
      method: "PUT",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({
        source_text: refs.sourceText.value,
        translated_text: refs.translatedText.value,
        source_lang: refs.sourceLanguage.value,
        target_lang: refs.targetLanguage.value,
      }),
    });
    state.activeMemoryRevision = memory.revision;
    resetMemoryEditing();
    updateMemoryControls();
    renderQa(memory.qa_warnings);
    setHistoryState("memory.saved", { revision: memory.revision });
    setTranslationMeta({
      kind: "asset",
      modelId: memory.model_id,
      modelLabel: memory.model_label,
      revision: memory.revision,
    });
    await loadHistory();
    showToast(t("memory.saved_done", { revision: memory.revision }));
  } catch (error) {
    showToast(error.message, true);
  }
}

function cancelMemoryEdit() {
  const revision = state.memoryEditSnapshot?.activeMemoryRevision || state.activeMemoryRevision;
  resetMemoryEditing(true);
  updateMemoryControls();
  showToast(t("memory.cancelled", { revision }));
}

function resetMemoryEditing(restore = false) {
  const snapshot = state.memoryEditSnapshot;
  state.editingMemory = false;
  state.memoryEditSnapshot = null;
  refs.translatedText.readOnly = true;
  refs.cancelMemoryEdit.hidden = true;
  if (restore && snapshot) restoreMemoryEditSnapshot(snapshot);
}

function captureMemoryEditSnapshot() {
  return {
    sourceText: refs.sourceText.value,
    translatedText: refs.translatedText.value,
    sourceLanguage: refs.sourceLanguage.value,
    targetLanguage: refs.targetLanguage.value,
    modelId: refs.modelSelect.value,
    activeRecordId: state.activeRecordId,
    activeMemoryId: state.activeMemoryId,
    activeMemoryRevision: state.activeMemoryRevision,
    historyMessage: {
      key: state.historyMessage.key,
      variables: { ...state.historyMessage.variables },
    },
    translationMeta: { ...state.translationMeta },
    qaWarnings: [...state.qaWarnings],
  };
}

function restoreMemoryEditSnapshot(snapshot) {
  state.activeRecordId = snapshot.activeRecordId;
  state.activeMemoryId = snapshot.activeMemoryId;
  state.activeMemoryRevision = snapshot.activeMemoryRevision;
  if ([...refs.modelSelect.options].some((option) => option.value === snapshot.modelId)) {
    refs.modelSelect.value = snapshot.modelId;
    state.previousModelId = snapshot.modelId;
  }
  populateLanguages(snapshot.sourceLanguage, snapshot.targetLanguage);
  try {
    localStorage.setItem("translator.target", snapshot.targetLanguage);
    if (state.config) {
      localStorage.setItem(`translator.model.${state.config.profile}`, snapshot.modelId);
    }
  } catch {
    // Workspace restoration does not depend on browser preference storage.
  }
  refs.sourceText.value = snapshot.sourceText;
  showTranslation(snapshot.translatedText, false);
  state.historyMessage = {
    key: snapshot.historyMessage.key,
    variables: { ...snapshot.historyMessage.variables },
  };
  state.translationMeta = { ...snapshot.translationMeta };
  renderHistoryState();
  renderTranslationMeta();
  renderQa(snapshot.qaWarnings);
  refs.deleteActive.hidden = !state.activeRecordId;
  updateCharacterCount();
  updateModelDescription();
  renderHistory();
}

function memoryEditIsDirty() {
  const snapshot = state.memoryEditSnapshot;
  if (!state.editingMemory || !snapshot) return false;
  return (
    refs.sourceText.value !== snapshot.sourceText ||
    refs.translatedText.value !== snapshot.translatedText ||
    refs.sourceLanguage.value !== snapshot.sourceLanguage ||
    refs.targetLanguage.value !== snapshot.targetLanguage ||
    refs.modelSelect.value !== snapshot.modelId
  );
}

function confirmDiscardMemoryEdit() {
  if (!state.editingMemory) return true;
  if (memoryEditIsDirty() && !window.confirm(t("memory.discard_confirm"))) return false;
  resetMemoryEditing();
  updateMemoryControls();
  return true;
}

function hasUnpersistedResult() {
  return refs.translatedText.classList.contains("visible") &&
    Boolean(refs.translatedText.value.trim()) &&
    !state.activeRecordId &&
    !state.activeMemoryId;
}

function confirmDiscardUnpersistedResult() {
  return !hasUnpersistedResult() || window.confirm(t("output.discard_unsaved_confirm"));
}

function handleBeforeUnload(event) {
  if (!memoryEditIsDirty() && !hasUnpersistedResult()) return;
  event.preventDefault();
  event.returnValue = "";
}

function updateMemoryControls() {
  if (state.editingMemory) return;
  refs.deleteMemory.hidden = !state.activeMemoryId;
  if (state.activeMemoryId) {
    refs.memoryAction.hidden = false;
    refs.memoryAction.textContent = t("memory.edit", { revision: state.activeMemoryRevision });
    return;
  }
  refs.memoryAction.hidden = !state.activeRecordId;
  refs.memoryAction.textContent = t("memory.approve");
}

async function deleteActiveMemory() {
  if (!state.activeMemoryId) return;
  if (!window.confirm(t("memory.delete_confirm"))) {
    return;
  }
  try {
    await api(`/api/memory/${encodeURIComponent(state.activeMemoryId)}`, { method: "DELETE" });
    const active = state.records.find(
      (record) => record.approved_memory_id === state.activeMemoryId || record.id === state.activeRecordId,
    );
    if (active) {
      active.approved_memory_id = null;
      active.approved_revision = null;
    }
    resetMemoryEditing();
    state.activeMemoryId = null;
    state.activeMemoryRevision = null;
    updateMemoryControls();
    setHistoryState(state.activeRecordId ? "memory.deleted_history_kept" : "memory.deleted");
    await loadHistory();
    showToast(t("memory.delete_done"));
  } catch (error) {
    showToast(error.message, true);
  }
}

async function deleteActiveRecord() {
  if (!state.activeRecordId) return;
  const confirmation = state.activeMemoryId
    ? t("history.delete_confirm_keep_asset")
    : t("history.delete_confirm");
  if (!window.confirm(confirmation)) return;
  if (!confirmDiscardMemoryEdit()) return;

  try {
    await api(`/api/history/${encodeURIComponent(state.activeRecordId)}`, { method: "DELETE" });
    state.activeRecordId = null;
    refs.deleteActive.hidden = true;
    resetMemoryEditing();
    updateMemoryControls();
    setHistoryState(state.activeMemoryId ? "history.deleted_asset_kept" : "history.deleted");
    await loadHistory();
    showToast(t("history.delete_done"));
  } catch (error) {
    showToast(error.message, true);
  }
}

function updateCharacterCount() {
  const count = [...refs.sourceText.value].length;
  const max = state.config?.max_text_chars;
  refs.characterCount.textContent = max
    ? t("source.character_count_max", { count: formatNumber(count), max: formatNumber(max) })
    : t("source.character_count", { count: formatNumber(count) });
}

function updateSaveState(updateStatus = true) {
  const saving = refs.saveHistory.checked;
  refs.saveHint.textContent = saving ? t("save.on") : t("save.off");
  if (updateStatus) setHistoryState(saving ? "history.save_on" : "history.private_mode");
}

function updateModelDescription() {
  const model = selectedModel();
  if (!model) return;
  const availability = modelIsAvailable(model) ? "" : ` · ${t("model.setup_required")}`;
  refs.modelDescription.textContent = `${localizedModelDescription(model)} · ${localizedPrivacy(model.privacy)}${availability}`;
  updatePrivacyBadge(model);
  updateTranslateAvailability();
}

function updatePrivacyBadge(model = selectedModel()) {
  const isPrivateNetwork = model?.privacy === "private_network";
  refs.privacyBadge.classList.toggle("private-network", isPrivateNetwork);
  refs.privacyBadgeText.textContent = model
    ? t(isPrivateNetwork ? "privacy.badge.private_network" : "privacy.badge.device")
    : t("privacy.badge.loading");
}

function updateTranslateAvailability() {
  const available = modelIsAvailable(selectedModel());
  refs.translateButton.disabled = state.translating || !available;
  if (available) {
    refs.translateButton.removeAttribute("title");
  } else {
    refs.translateButton.title = t("translate.model_unavailable");
  }
}

function ensurePrivateNetworkConsent(model) {
  if (model?.privacy !== "private_network" || state.privateNetworkConsent.has(model.id)) return true;
  if (!window.confirm(t("privacy.private_network_confirm"))) return false;
  state.privateNetworkConsent.add(model.id);
  return true;
}

function handleModelChange() {
  const source = refs.sourceLanguage.value;
  const target = refs.targetLanguage.value;
  const model = selectedModel();
  const previous = state.previousModelId;

  if (!modelIsAvailable(model) || !ensurePrivateNetworkConsent(model)) {
    if ([...refs.modelSelect.options].some((option) => option.value === previous)) {
      refs.modelSelect.value = previous;
    }
    populateLanguages(source, target);
    updateModelDescription();
    if (!modelIsAvailable(model)) showToast(t("translate.model_unavailable"), true);
    return;
  }

  state.previousModelId = model.id;
  if (state.config) {
    try {
      localStorage.setItem(`translator.model.${state.config.profile}`, model.id);
    } catch {
      // Model selection remains usable when browser storage is disabled.
    }
  }
  populateLanguages(source, target);
  updateModelDescription();
}

function swapLanguages() {
  if (!confirmDiscardMemoryEdit()) return;
  if (!confirmDiscardUnpersistedResult()) return;
  const source = refs.sourceLanguage.value;
  const target = refs.targetLanguage.value;
  refs.sourceLanguage.value = source === "auto" ? target : target;
  refs.targetLanguage.value = source === "auto" ? (target === "ko" ? "en" : "ko") : source;
  const sourceText = refs.sourceText.value;
  const translatedText = refs.translatedText.value;
  if (translatedText.trim()) {
    refs.sourceText.value = translatedText;
    showTranslation(sourceText, false);
  }
  updateCharacterCount();
}

function clearWorkspace() {
  if (!confirmDiscardMemoryEdit()) return;
  if (!confirmDiscardUnpersistedResult()) return;
  refs.sourceText.value = "";
  state.activeRecordId = null;
  state.activeMemoryId = null;
  state.activeMemoryRevision = null;
  refs.deleteActive.hidden = true;
  updateMemoryControls();
  setEmptyOutput(t("output.empty_title"), t("output.empty_subtitle"));
  setHistoryState(refs.saveHistory.checked ? "history.save_on" : "history.private_mode");
  renderQa([]);
  updateCharacterCount();
  refs.sourceText.focus();
  renderHistory();
}

async function copyOutput() {
  const text = refs.translatedText.value;
  if (!text) {
    showToast(t("copy.empty"), true);
    return;
  }
  try {
    await navigator.clipboard.writeText(text);
    showToast(t("copy.done"));
  } catch {
    refs.translatedText.select();
    document.execCommand("copy");
    showToast(t("copy.done"));
  }
}

function openHistoryPanel() {
  if (!historyDrawerMedia.matches) return;
  state.historyReturnFocus = document.activeElement;
  refs.historyPanel.classList.add("open");
  refs.mobileScrim.classList.add("show");
  document.body.classList.add("drawer-open");
  syncHistoryPanelAccessibility();
  window.requestAnimationFrame(() => refs.historySearch.focus());
}

function closeHistoryPanel() {
  const wasOpen = refs.historyPanel.classList.contains("open");
  refs.historyPanel.classList.remove("open");
  refs.mobileScrim.classList.remove("show");
  document.body.classList.remove("drawer-open");
  syncHistoryPanelAccessibility();
  if (wasOpen && state.historyReturnFocus instanceof HTMLElement && state.historyReturnFocus.isConnected) {
    state.historyReturnFocus.focus();
  }
  state.historyReturnFocus = null;
}

function syncHistoryPanelAccessibility() {
  const mobile = historyDrawerMedia.matches;
  if (!mobile) {
    refs.historyPanel.classList.remove("open");
    refs.mobileScrim.classList.remove("show");
    document.body.classList.remove("drawer-open");
  }
  const open = mobile && refs.historyPanel.classList.contains("open");
  refs.historyToggle.setAttribute("aria-expanded", String(open));
  refs.historyPanel.setAttribute("aria-hidden", String(mobile && !open));
  refs.historyPanel.toggleAttribute("inert", mobile && !open);
  if (mobile) {
    refs.historyPanel.setAttribute("role", "dialog");
    refs.historyPanel.setAttribute("aria-modal", String(open));
  } else {
    refs.historyPanel.removeAttribute("role");
    refs.historyPanel.removeAttribute("aria-modal");
  }
  for (const surface of [refs.topbar, refs.workspace]) {
    surface.toggleAttribute("inert", open);
    if (open) surface.setAttribute("aria-hidden", "true");
    else surface.removeAttribute("aria-hidden");
  }
}

function handleHistoryPanelKeydown(event) {
  if (!historyDrawerMedia.matches || !refs.historyPanel.classList.contains("open")) return;
  if (event.key === "Escape") {
    event.preventDefault();
    closeHistoryPanel();
    return;
  }
  if (event.key !== "Tab") return;

  const focusable = [...refs.historyPanel.querySelectorAll(
    'button:not([disabled]), input:not([disabled]), select:not([disabled]), textarea:not([disabled]), [href], [tabindex]:not([tabindex="-1"])',
  )].filter((element) => element.offsetParent !== null);
  if (!focusable.length) {
    event.preventDefault();
    refs.historyPanel.focus();
    return;
  }
  const first = focusable[0];
  const last = focusable[focusable.length - 1];
  if (event.shiftKey && (document.activeElement === first || !refs.historyPanel.contains(document.activeElement))) {
    event.preventDefault();
    last.focus();
  } else if (!event.shiftKey && document.activeElement === last) {
    event.preventDefault();
    first.focus();
  }
}

function emptyMessage(message) {
  const element = document.createElement("div");
  element.className = "history-empty";
  element.textContent = message;
  return element;
}

function languageLabel(code) {
  return localizedLanguageName(code);
}

function formatDay(timestamp) {
  const date = new Date(timestamp);
  const today = new Date();
  const yesterday = new Date();
  yesterday.setDate(today.getDate() - 1);
  if (date.toDateString() === today.toDateString()) return t("date.today");
  if (date.toDateString() === yesterday.toDateString()) return t("date.yesterday");
  return new Intl.DateTimeFormat(i18n.getLocale(), { month: "long", day: "numeric" }).format(date);
}

function formatTime(timestamp) {
  return new Intl.DateTimeFormat(i18n.getLocale(), { hour: "2-digit", minute: "2-digit" }).format(
    new Date(timestamp),
  );
}

function formatLatency(milliseconds) {
  if (milliseconds < 1000) return t("latency.ms", { value: formatNumber(milliseconds) });
  return t("latency.seconds", {
    value: new Intl.NumberFormat(i18n.getLocale(), { maximumFractionDigits: 1 }).format(milliseconds / 1000),
  });
}

function formatNumber(value) {
  return new Intl.NumberFormat(i18n.getLocale()).format(value);
}

function showToast(message, error = false) {
  window.clearTimeout(state.toastTimer);
  refs.toast.textContent = message;
  refs.toast.classList.toggle("error", error);
  refs.toast.classList.add("show");
  state.toastTimer = window.setTimeout(() => refs.toast.classList.remove("show"), 3200);
}

function announceTranslation(message) {
  refs.translationAnnouncer.textContent = "";
  window.requestAnimationFrame(() => {
    refs.translationAnnouncer.textContent = message;
  });
}

async function api(path, options = {}) {
  const headers = new Headers(options.headers || {});
  if (state.sessionToken) headers.set("Authorization", `Bearer ${state.sessionToken}`);
  const response = await fetch(path, {
    cache: "no-store",
    ...options,
    credentials: "omit",
    headers,
  });
  if (response.status === 204) return null;
  const contentType = response.headers.get("content-type") || "";
  const body = contentType.includes("application/json") ? await response.json() : null;
  if (!response.ok) {
    const code = body?.error?.code;
    const fallback = body?.error?.message || t("error.generic", { status: response.status });
    const error = new Error(code ? t(`error.${code}`, { status: response.status }, fallback) : fallback);
    error.code = code;
    throw error;
  }
  return body;
}
