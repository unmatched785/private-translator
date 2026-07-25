const capabilitiesElement = document.querySelector("#capabilities");
const statusElement = document.querySelector("#status");
const progressElement = document.querySelector("#progress");
const resultsElement = document.querySelector("#results");
const buttons = [...document.querySelectorAll("button")];

const corpus = await fetch("./corpus.json").then((response) => response.json());

async function availability(sourceLanguage, targetLanguage) {
  if (!("Translator" in self)) return "unsupported";
  try {
    return await Translator.availability({ sourceLanguage, targetLanguage });
  } catch (error) {
    return `error: ${error.name}: ${error.message}`;
  }
}

capabilitiesElement.textContent = JSON.stringify(
  {
    userAgent: navigator.userAgent,
    translatorApi: "Translator" in self,
    webGpu: "gpu" in navigator,
    crossOriginIsolated,
    koToEn: await availability("ko", "en"),
    enToKo: await availability("en", "ko"),
  },
  null,
  2,
);

async function runPair(sourceLanguage, targetLanguage) {
  buttons.forEach((button) => {
    button.disabled = true;
  });
  resultsElement.replaceChildren();
  progressElement.value = 0;
  statusElement.textContent = `${sourceLanguage} → ${targetLanguage} 준비 중`;

  try {
    if (!("Translator" in self)) {
      throw new Error("이 Chrome에서는 Translator API를 사용할 수 없습니다.");
    }

    const translator = await Translator.create({
      sourceLanguage,
      targetLanguage,
      monitor(monitor) {
        monitor.addEventListener("downloadprogress", (event) => {
          progressElement.value = event.loaded;
          statusElement.textContent = `언어팩 다운로드 ${Math.round(event.loaded * 100)}%`;
        });
      },
    });

    const selected = corpus.filter(
      (testCase) =>
        testCase.source === sourceLanguage && testCase.target === targetLanguage,
    );
    const allResults = [];

    for (const [index, testCase] of selected.entries()) {
      statusElement.textContent = `${index + 1}/${selected.length} · ${testCase.id}`;
      const started = performance.now();
      const translatedText = await translator.translate(testCase.text);
      const elapsedMs = Math.round(performance.now() - started);
      allResults.push({ ...testCase, translatedText, elapsedMs });

      const article = document.createElement("article");
      const id = document.createElement("div");
      id.className = "id";
      id.textContent = `${testCase.id} · ${elapsedMs} ms`;
      const source = document.createElement("p");
      source.textContent = testCase.text;
      const translation = document.createElement("p");
      translation.className = "translation";
      translation.textContent = translatedText;
      article.append(id, source, translation);
      resultsElement.append(article);
    }

    globalThis.chromeTranslatorResults ??= {};
    globalThis.chromeTranslatorResults[`${sourceLanguage}-${targetLanguage}`] = {
      generatedAt: new Date().toISOString(),
      results: allResults,
    };
    progressElement.value = 1;
    statusElement.textContent = `완료 · ${selected.length}문장`;
  } catch (error) {
    statusElement.className = "error";
    statusElement.textContent = `${error.name}: ${error.message}`;
  } finally {
    buttons.forEach((button) => {
      button.disabled = false;
    });
  }
}

document.querySelector("#run-ko-en").addEventListener("click", () => runPair("ko", "en"));
document.querySelector("#run-en-ko").addEventListener("click", () => runPair("en", "ko"));
