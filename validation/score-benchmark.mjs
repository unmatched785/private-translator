import { readFile, writeFile } from "node:fs/promises";

const readJson = async (relativePath) =>
  JSON.parse(await readFile(new URL(relativePath, import.meta.url), "utf8"));

const corpus = await readJson("./corpus.json");
const hymt2File = await readJson("./results/hymt2-local.json");
const chromeFile = await readJson("./results/chrome-translator.json");
const judgmentsFile = await readJson("./judgments.json");

const hymt2Results = new Map(hymt2File.results.map((result) => [result.id, result]));
const chromeResults = new Map(chromeFile.results.map((result) => [result.id, result]));
const judgments = new Map(judgmentsFile.cases.map((judgment) => [judgment.id, judgment]));

const rows = corpus.map((testCase) => {
  const hymt2 = hymt2Results.get(testCase.id);
  const chrome = chromeResults.get(testCase.id);
  const judgment = judgments.get(testCase.id);
  if (!hymt2 || !chrome || !judgment) {
    throw new Error(`Missing evidence for ${testCase.id}`);
  }
  return {
    ...testCase,
    hymt2: {
      translation: hymt2.translatedText,
      elapsedMs: hymt2.elapsedMs,
      score: judgment.hymt2[0],
      note: judgment.hymt2[1],
    },
    chrome: {
      translation: chrome.translation,
      elapsedMs: chrome.elapsedMs,
      score: judgment.chrome[0],
      note: judgment.chrome[1],
    },
  };
});

const summarize = (engine) => {
  const times = rows.map((row) => row[engine].elapsedMs).sort((a, b) => a - b);
  const score = rows.reduce((sum, row) => sum + row[engine].score, 0);
  const failures = rows.filter((row) => row[engine].score === 0).length;
  return {
    score,
    maxScore: rows.length * 2,
    scorePercent: Math.round((score / (rows.length * 2)) * 1000) / 10,
    majorFailures: failures,
    meanMs: Math.round(times.reduce((sum, value) => sum + value, 0) / times.length),
    p95Ms: times[Math.ceil(times.length * 0.95) - 1],
  };
};

const summary = {
  generatedAt: new Date().toISOString(),
  methodology: {
    cases: rows.length,
    directions: ["ko-en", "en-ko"],
    scoring: judgmentsFile.rubric,
    limitation:
      "Scores are a first-pass human semantic review by the project evaluator, not an independent blinded panel.",
  },
  hymt2: summarize("hymt2"),
  chrome: summarize("chrome"),
};

await writeFile(
  new URL("./results/comparison.json", import.meta.url),
  `${JSON.stringify({ summary, rows }, null, 2)}\n`,
);

const markdown = [
  "# Translation viability comparison",
  "",
  `Generated: ${summary.generatedAt}`,
  "",
  "## Summary",
  "",
  "| Engine | Semantic score | Major failures | Mean latency | p95 latency |",
  "| --- | ---: | ---: | ---: | ---: |",
  `| Hy-MT2 1.8B Q4 | ${summary.hymt2.score}/${summary.hymt2.maxScore} (${summary.hymt2.scorePercent}%) | ${summary.hymt2.majorFailures} | ${summary.hymt2.meanMs} ms | ${summary.hymt2.p95Ms} ms |`,
  `| Chrome on-device Translator | ${summary.chrome.score}/${summary.chrome.maxScore} (${summary.chrome.scorePercent}%) | ${summary.chrome.majorFailures} | ${summary.chrome.meanMs} ms | ${summary.chrome.p95Ms} ms |`,
  "",
  "Scoring: 2 = usable; 1 = meaning mostly survives with a weakness; 0 = major semantic failure.",
  "",
  `Limitation: ${summary.methodology.limitation}`,
  "",
  "## Case results",
  "",
  ...rows.flatMap((row) => [
    `### ${row.id} · ${row.category}`,
    "",
    `Source: ${row.text.replaceAll("\n", "  \n")}`,
    "",
    `- Hy-MT2 (${row.hymt2.score}/2, ${row.hymt2.elapsedMs} ms): ${row.hymt2.translation.replaceAll("\n", "  \n")}`,
    `  - ${row.hymt2.note}`,
    `- Chrome (${row.chrome.score}/2, ${row.chrome.elapsedMs} ms): ${row.chrome.translation.replaceAll("\n", "  \n")}`,
    `  - ${row.chrome.note}`,
    "",
  ]),
];

await writeFile(new URL("./results/comparison.md", import.meta.url), `${markdown.join("\n")}\n`);
process.stdout.write(`${JSON.stringify(summary, null, 2)}\n`);
