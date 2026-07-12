(() => {
  const DEFAULT_LOCALE = "en";
  const SUPPORTED_LOCALES = ["en", "ko"];
  const STORAGE_KEY = "translator.ui_locale";

  const messages = {
    en: {
      "meta.description": "A private, offline translator that keeps translations and history on your device",
      "locale.label": "Interface language",
      "brand.subtitle": "Translate on your device. Keep your history private.",
      "status.loading": "Loading",
      "status.connection_error": "Connection error",
      "status.no_external": "No external transfer",
      "history.aria": "Translation history",
      "history.title": "Translation history",
      "history.close": "Close translation history",
      "history.open": "Open translation history",
      "history.search": "Search source text and translations",
      "history.filters": "History filters",
      "history.filter.all": "All",
      "history.filter.favorites": "★ Favorites",
      "history.filter.assets": "✓ Translation assets",
      "history.loading": "Opening encrypted vault…",
      "history.empty.search": "No matching results.\nYour records remain encrypted on this device.",
      "history.empty.favorites": "No favorite translations yet.",
      "history.empty.assets": "No approved translation assets yet.\nOpen a good translation and approve it as an asset.",
      "history.empty.all": "No saved translations yet.\nPaste your first text to begin.",
      "history.load_failed": "Could not load translation history.",
      "history.open_record": "Open translation record: {text}",
      "history.favorite.add": "Add to favorites",
      "history.favorite.remove": "Remove from favorites",
      "history.saved": "Saved to encrypted history",
      "history.not_saved": "This translation was not saved",
      "history.opened": "Opened from encrypted history",
      "history.save_on": "History saving is on",
      "history.private_mode": "Private translation mode",
      "history.delete": "Delete history",
      "history.delete_confirm": "Delete this translation record from the local vault?",
      "history.delete_confirm_keep_asset": "Delete only this translation record? The approved asset and its revisions will remain.",
      "history.deleted": "History record deleted",
      "history.deleted_asset_kept": "History deleted · translation asset kept",
      "history.delete_done": "Translation record deleted.",
      "vault.protected": "Protected by your Windows account",
      "vault.note": "Source text and translations are stored only in the encrypted local vault.",
      "hero.title": "Paste and translate instantly",
      "hero.subtitle": "No account, upload, or send button required.",
      "save.title": "Save history",
      "save.on": "Encrypt and keep on this device",
      "save.off": "Do not save this translation",
      "language.source": "Source",
      "language.target": "Translate to",
      "language.model": "Model",
      "language.source_aria": "Source language",
      "language.target_aria": "Target language",
      "language.swap": "Swap languages",
      "language.model_aria": "Translation model",
      "language.auto": "Detect language",
      "source.clear": "Clear",
      "source.placeholder": "Paste or type text to translate",
      "source.paste_hint": "Auto-translate on paste · Ctrl+Enter",
      "source.character_count": "{count} characters",
      "source.character_count_max": "{count} / {max} characters",
      "output.title": "Translation",
      "output.copy": "Copy",
      "output.empty_title": "Your translation will appear here",
      "output.empty_subtitle": "Everything is processed on the selected private device.",
      "output.aria": "Translation result",
      "output.ready": "Ready",
      "output.processing": "Processing",
      "output.loading_title": "The local model is translating",
      "output.loading_subtitle": "Your text is not sent outside the selected private device.",
      "output.engine_failed": "Could not connect to the translation engine",
      "output.chunk_count": "{count} chunks",
      "translate.button": "Translate",
      "translate.loading": "Translating…",
      "translate.empty": "Enter text to translate.",
      "translate.not_ready": "The local translation service is not ready yet.",
      "translate.too_long": "The text exceeds the {max}-character limit.",
      "copy.empty": "There is no translation to copy.",
      "copy.done": "Translation copied.",
      "qa.title": "Review recommended: ",
      "qa.empty_result": "The translation result is empty",
      "qa.number_mismatch": "Numbers or units may have changed",
      "qa.url_mismatch": "A URL may be missing or changed",
      "privacy.device": "Processed on this device",
      "privacy.private_network": "Processed on a private network device",
      "privacy.footer_label": "Local processing:",
      "privacy.footer_text": "Translation content is never placed in URLs, logs, or analytics.",
      "profile.records": "{profile} · {count} records",
      "profile.assets": "{profile} · {count} assets",
      "date.today": "Today",
      "date.yesterday": "Yesterday",
      "latency.ms": "{value} ms",
      "latency.seconds": "{value} s",
      "memory.approve": "Approve as asset",
      "memory.edit": "Edit asset v{revision}",
      "memory.save_revision": "Save as v{revision}",
      "memory.cancel": "Cancel edit",
      "memory.delete": "Delete asset",
      "memory.opened": "Opened approved translation asset",
      "memory.approved": "Approved as translation asset · v{revision}",
      "memory.approve_done": "Approved as a reusable local translation asset.",
      "memory.editing": "Editing translation asset · saving creates a new revision",
      "memory.saved": "Translation asset revision saved · v{revision}",
      "memory.saved_done": "Kept the previous content and added v{revision}.",
      "memory.cancelled": "Translation asset v{revision} · edit cancelled",
      "memory.delete_confirm": "Delete this approved asset and every revision? The original history record will remain.",
      "memory.deleted": "Translation asset deleted",
      "memory.deleted_history_kept": "Translation asset deleted · history kept",
      "memory.delete_done": "Deleted the approved asset and all revisions.",
      "memory.meta": "asset v{revision}",
      "model.loading": "Loading model information.",
      "model.hy-mt2-1.8b-q4.label": "Hy-MT2 1.8B · Fast local",
      "model.hy-mt2-1.8b-q4.description": "A lightweight CPU model for everyday laptops",
      "model.hy-mt2-7b-q4.label": "Hy-MT2 7B · High quality local",
      "model.hy-mt2-7b-q4.description": "Requires the optional 7B model pack or a private server",
      "model.translategemma-12b-q4.label": "TranslateGemma 12B · Experimental",
      "model.translategemma-12b-q4.description": "Experimental adapter target; requires a private server",
      "model.local-demo.label": "Local demo engine",
      "model.local-demo.description": "Development engine for testing the interface and vault",
      "model.hy-mt2-7b-preview.label": "Hy-MT2 7B · Interface preview",
      "model.hy-mt2-7b-preview.description": "Preview entry for the Quality profile model selector",
      "error.local_only": "This app can only be used from its local address.",
      "error.empty_text": "Enter text to translate.",
      "error.text_too_long": "The text is too long for one request.",
      "error.target_required": "Select a target language.",
      "error.unsupported_language": "The selected model does not support one of these languages.",
      "error.model_not_found": "The selected model is not available in this profile.",
      "error.invalid_request_id": "The translation request identifier is invalid.",
      "error.request_superseded": "A newer translation request replaced this one.",
      "error.history_not_found": "The translation record was not found.",
      "error.memory_not_found": "The translation asset was not found.",
      "error.empty_memory": "Enter both source text and translated text.",
      "error.memory_text_too_long": "The translation asset is too long.",
      "error.engine_unavailable": "The local translation engine could not complete the request.",
      "error.storage_error": "The encrypted local vault could not complete the operation.",
      "error.route_not_found": "The requested path was not found.",
      "error.vault_open": "Could not open the encrypted local vault.",
      "error.generic": "The request could not be completed ({status}).",
    },
    ko: {
      "meta.description": "내 장치 안에서만 번역하고 기록하는 로컬 번역기",
      "locale.label": "화면 언어",
      "brand.subtitle": "내 장치 안에서 번역하고, 내 기록으로 남깁니다",
      "status.loading": "불러오는 중",
      "status.connection_error": "연결 오류",
      "status.no_external": "외부 전송 없음",
      "history.aria": "번역 기록",
      "history.title": "번역 기록",
      "history.close": "번역 기록 닫기",
      "history.open": "번역 기록 열기",
      "history.search": "원문과 번역문 검색",
      "history.filters": "기록 필터",
      "history.filter.all": "전체",
      "history.filter.favorites": "★ 즐겨찾기",
      "history.filter.assets": "✓ 번역 자산",
      "history.loading": "암호화 기록고를 여는 중…",
      "history.empty.search": "검색 결과가 없습니다.\n기록은 암호화된 상태로 로컬에 남아 있습니다.",
      "history.empty.favorites": "즐겨찾기한 번역이 없습니다.",
      "history.empty.assets": "아직 승인한 번역 자산이 없습니다.\n좋은 번역을 열어 자산으로 승인하세요.",
      "history.empty.all": "아직 저장된 번역이 없습니다.\n첫 번역을 붙여넣어 시작하세요.",
      "history.load_failed": "기록을 불러올 수 없습니다.",
      "history.open_record": "{text} 번역 기록 열기",
      "history.favorite.add": "즐겨찾기 추가",
      "history.favorite.remove": "즐겨찾기 해제",
      "history.saved": "암호화 기록 저장됨",
      "history.not_saved": "이번 번역은 기록하지 않음",
      "history.opened": "암호화 기록에서 열림",
      "history.save_on": "기록 저장 켜짐",
      "history.private_mode": "비공개 번역 모드",
      "history.delete": "기록 삭제",
      "history.delete_confirm": "이 번역 기록을 로컬 기록고에서 삭제할까요?",
      "history.delete_confirm_keep_asset": "이 번역 기록만 삭제할까요? 승인한 번역 자산과 버전은 그대로 유지됩니다.",
      "history.deleted": "기록 삭제됨",
      "history.deleted_asset_kept": "기록 삭제됨 · 번역 자산은 유지됨",
      "history.delete_done": "번역 기록을 삭제했습니다.",
      "vault.protected": "Windows 계정으로 보호됨",
      "vault.note": "원문과 번역문은 암호화된 로컬 기록고에만 저장됩니다.",
      "hero.title": "붙여넣으면 바로 번역됩니다",
      "hero.subtitle": "계정도, 업로드도, 전송 버튼도 필요 없습니다.",
      "save.title": "기록 저장",
      "save.on": "암호화하여 로컬에 보관",
      "save.off": "이번 번역은 저장하지 않음",
      "language.source": "원문",
      "language.target": "번역",
      "language.model": "모델",
      "language.source_aria": "원문 언어",
      "language.target_aria": "번역 언어",
      "language.swap": "언어 맞바꾸기",
      "language.model_aria": "번역 모델",
      "language.auto": "언어 자동 감지",
      "source.clear": "지우기",
      "source.placeholder": "번역할 내용을 여기에 붙여넣으세요",
      "source.paste_hint": "붙여넣기 시 자동 번역 · Ctrl+Enter",
      "source.character_count": "{count}자",
      "source.character_count_max": "{count} / {max}자",
      "output.title": "번역문",
      "output.copy": "복사",
      "output.empty_title": "번역 결과가 여기에 표시됩니다",
      "output.empty_subtitle": "모든 처리는 선택한 개인 장치 안에서 이루어집니다.",
      "output.aria": "번역 결과",
      "output.ready": "준비됨",
      "output.processing": "처리 중",
      "output.loading_title": "로컬 모델이 번역하고 있습니다",
      "output.loading_subtitle": "텍스트는 선택한 개인 장치 밖으로 전송되지 않습니다.",
      "output.engine_failed": "번역 엔진에 연결하지 못했습니다",
      "output.chunk_count": "{count}개 구간",
      "translate.button": "번역하기",
      "translate.loading": "번역 중…",
      "translate.empty": "번역할 내용을 입력하세요.",
      "translate.not_ready": "로컬 번역 서비스가 아직 준비되지 않았습니다.",
      "translate.too_long": "텍스트가 최대 {max}자를 초과했습니다.",
      "copy.empty": "복사할 번역문이 없습니다.",
      "copy.done": "번역문을 복사했습니다.",
      "qa.title": "확인 권장: ",
      "qa.empty_result": "번역 결과가 비어 있습니다",
      "qa.number_mismatch": "원문의 숫자 또는 단위가 번역문과 다를 수 있습니다",
      "qa.url_mismatch": "URL이 누락되거나 변경되었을 수 있습니다",
      "privacy.device": "이 장치에서 처리",
      "privacy.private_network": "개인 네트워크 장치에서 처리",
      "privacy.footer_label": "로컬 처리:",
      "privacy.footer_text": "번역 내용은 주소, 로그, 분석 도구에 남기지 않습니다.",
      "profile.records": "{profile} · 기록 {count}개",
      "profile.assets": "{profile} · 자산 {count}개",
      "date.today": "오늘",
      "date.yesterday": "어제",
      "latency.ms": "{value}ms",
      "latency.seconds": "{value}초",
      "memory.approve": "번역 자산으로 승인",
      "memory.edit": "번역 자산 v{revision} 수정",
      "memory.save_revision": "v{revision}로 저장",
      "memory.cancel": "수정 취소",
      "memory.delete": "자산 삭제",
      "memory.opened": "승인 번역 자산에서 열림",
      "memory.approved": "번역 자산으로 승인됨 · v{revision}",
      "memory.approve_done": "이 번역을 재사용 가능한 로컬 자산으로 승인했습니다.",
      "memory.editing": "번역 자산 수정 중 · 저장하면 새 버전이 추가됩니다",
      "memory.saved": "번역 자산 수정 저장됨 · v{revision}",
      "memory.saved_done": "이전 내용은 유지하고 v{revision}을 추가했습니다.",
      "memory.cancelled": "번역 자산 v{revision} · 수정 취소됨",
      "memory.delete_confirm": "이 승인 번역 자산과 모든 이전 버전을 삭제할까요? 원본 번역 기록은 유지됩니다.",
      "memory.deleted": "번역 자산 삭제됨",
      "memory.deleted_history_kept": "번역 자산 삭제됨 · 기록은 유지됨",
      "memory.delete_done": "승인 번역 자산과 버전을 삭제했습니다.",
      "memory.meta": "번역 자산 v{revision}",
      "model.loading": "모델 정보를 불러오는 중입니다.",
      "model.hy-mt2-1.8b-q4.label": "Hy-MT2 1.8B · 빠른 로컬",
      "model.hy-mt2-1.8b-q4.description": "일반 사무용 노트북 CPU를 위한 기본 모델",
      "model.hy-mt2-7b-q4.label": "Hy-MT2 7B · 고품질 로컬",
      "model.hy-mt2-7b-q4.description": "추가 7B 모델팩 또는 개인 서버 필요",
      "model.translategemma-12b-q4.label": "TranslateGemma 12B · 실험적",
      "model.translategemma-12b-q4.description": "전용 어댑터와 개인 서버가 필요한 실험 후보",
      "model.local-demo.label": "로컬 데모 엔진",
      "model.local-demo.description": "화면과 기록 동작을 확인하는 개발용 엔진",
      "model.hy-mt2-7b-preview.label": "Hy-MT2 7B · 선택 화면 미리보기",
      "model.hy-mt2-7b-preview.description": "Quality 프로필의 모델 선택 화면을 확인하는 항목",
      "error.local_only": "로컬 주소에서만 사용할 수 있습니다.",
      "error.empty_text": "번역할 텍스트를 입력하세요.",
      "error.text_too_long": "한 번에 번역할 수 있는 길이를 초과했습니다.",
      "error.target_required": "대상 언어를 선택하세요.",
      "error.unsupported_language": "선택한 모델이 원문 또는 대상 언어를 지원하지 않습니다.",
      "error.model_not_found": "선택한 모델이 현재 프로필에 없습니다.",
      "error.invalid_request_id": "번역 요청 식별자가 올바르지 않습니다.",
      "error.request_superseded": "더 최신 번역 요청으로 교체되었습니다.",
      "error.history_not_found": "번역 기록을 찾을 수 없습니다.",
      "error.memory_not_found": "번역 자산을 찾을 수 없습니다.",
      "error.empty_memory": "원문과 번역문을 모두 입력하세요.",
      "error.memory_text_too_long": "번역 자산의 길이가 너무 깁니다.",
      "error.engine_unavailable": "로컬 번역 엔진이 요청을 완료하지 못했습니다.",
      "error.storage_error": "암호화 로컬 기록고가 작업을 완료하지 못했습니다.",
      "error.route_not_found": "요청한 경로를 찾을 수 없습니다.",
      "error.vault_open": "로컬 기록고를 열 수 없습니다.",
      "error.generic": "요청을 처리할 수 없습니다 ({status}).",
    },
  };

  function supportedLocale(value) {
    const language = String(value || "").toLowerCase().split(/[-_]/)[0];
    return SUPPORTED_LOCALES.includes(language) ? language : null;
  }

  function normalizeLocale(value) {
    return supportedLocale(value) || DEFAULT_LOCALE;
  }

  function detectLocale() {
    try {
      const stored = localStorage.getItem(STORAGE_KEY);
      if (stored) return normalizeLocale(stored);
    } catch {
      // Local storage is optional; locale detection still works without it.
    }
    const preferences = Array.isArray(navigator.languages) ? navigator.languages : [navigator.language];
    return preferences.map(supportedLocale).find(Boolean) || DEFAULT_LOCALE;
  }

  let locale = detectLocale();

  function t(key, variables = {}, fallback = key) {
    const template = messages[locale]?.[key] ?? messages[DEFAULT_LOCALE]?.[key] ?? fallback;
    return String(template).replace(/\{([a-zA-Z0-9_]+)\}/g, (match, name) =>
      Object.hasOwn(variables, name) ? String(variables[name]) : match,
    );
  }

  function applyDocument(root = document) {
    document.documentElement.lang = locale;
    root.querySelectorAll("[data-i18n]").forEach((element) => {
      element.textContent = t(element.dataset.i18n);
    });
    root.querySelectorAll("[data-i18n-placeholder]").forEach((element) => {
      element.setAttribute("placeholder", t(element.dataset.i18nPlaceholder));
    });
    root.querySelectorAll("[data-i18n-aria-label]").forEach((element) => {
      element.setAttribute("aria-label", t(element.dataset.i18nAriaLabel));
    });
    root.querySelectorAll("[data-i18n-content]").forEach((element) => {
      element.setAttribute("content", t(element.dataset.i18nContent));
    });
  }

  function setLocale(value) {
    const next = normalizeLocale(value);
    locale = next;
    try {
      localStorage.setItem(STORAGE_KEY, next);
    } catch {
      // The app remains usable when browser storage is disabled.
    }
    applyDocument();
    window.dispatchEvent(new CustomEvent("translator:locale-change", { detail: { locale: next } }));
  }

  globalThis.TranslatorI18n = Object.freeze({
    applyDocument,
    getLocale: () => locale,
    messages,
    setLocale,
    supportedLocales: [...SUPPORTED_LOCALES],
    t,
  });
})();
