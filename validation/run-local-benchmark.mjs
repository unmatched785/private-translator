import { mkdir, readFile, writeFile } from "node:fs/promises";
import { performance } from "node:perf_hooks";

const endpoint = process.env.TRANSLATOR_BENCHMARK_ENDPOINT ?? "http://127.0.0.1:8189/v1";
const model = process.env.TRANSLATOR_BENCHMARK_MODEL ?? "hy-mt2-1.8b";
const corpusPath = new URL("./corpus.json", import.meta.url);
const outputDir = new URL("./results/", import.meta.url);
const cases = JSON.parse(await readFile(corpusPath, "utf8"));

const languageName = {
  ko: "Korean",
  en: "English",
};

function buildPrompt(testCase) {
  return [
    `Translate the following text from ${languageName[testCase.source]} into ${languageName[testCase.target]}. Note that you must ONLY output the translated result without any additional explanation:`,
    "",
    testCase.text,
  ].join("\n");
}

async function translate(testCase) {
  const started = performance.now();
  const response = await fetch(`${endpoint}/chat/completions`, {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({
      model,
      messages: [{ role: "user", content: buildPrompt(testCase) }],
      temperature: 0.7,
      top_p: 0.6,
      top_k: 20,
      repeat_penalty: 1.05,
      seed: 42,
      max_tokens: 512,
      stream: false,
    }),
  });

  const body = await response.json();
  if (!response.ok) {
    throw new Error(`${testCase.id}: ${response.status} ${JSON.stringify(body)}`);
  }

  return {
    ...testCase,
    engine: "Hy-MT2-1.8B-Q4_K_M",
    translatedText: body.choices?.[0]?.message?.content?.trim() ?? "",
    finishReason: body.choices?.[0]?.finish_reason ?? null,
    elapsedMs: Math.round(performance.now() - started),
    usage: body.usage ?? null,
  };
}

const results = [];
for (const [index, testCase] of cases.entries()) {
  process.stdout.write(`[${index + 1}/${cases.length}] ${testCase.id} ... `);
  const result = await translate(testCase);
  results.push(result);
  process.stdout.write(`${result.elapsedMs} ms\n${result.translatedText}\n\n`);
}

await mkdir(outputDir, { recursive: true });
await writeFile(
  new URL("./hymt2-local.json", outputDir),
  `${JSON.stringify(
    {
      generatedAt: new Date().toISOString(),
      endpoint,
      model,
      settings: {
        temperature: 0.7,
        topP: 0.6,
        topK: 20,
        repeatPenalty: 1.05,
        seed: 42,
        maxTokens: 512,
      },
      results,
    },
    null,
    2,
  )}\n`,
);

const markdown = [
  "# Hy-MT2 local benchmark",
  "",
  `Generated: ${new Date().toISOString()}`,
  "",
  ...results.flatMap((result) => [
    `## ${result.id} · ${result.category} · ${result.elapsedMs} ms`,
    "",
    `**Source (${result.source}):** ${result.text.replaceAll("\n", "  \n")}`,
    "",
    `**Translation (${result.target}):** ${result.translatedText.replaceAll("\n", "  \n")}`,
    "",
    `**Checks:** ${result.checks.join("; ")}`,
    "",
  ]),
];
await writeFile(new URL("./hymt2-local.md", outputDir), `${markdown.join("\n")}\n`);

const elapsed = results.map((result) => result.elapsedMs).sort((a, b) => a - b);
const total = elapsed.reduce((sum, value) => sum + value, 0);
const percentile = (p) => elapsed[Math.min(elapsed.length - 1, Math.ceil(elapsed.length * p) - 1)];
process.stdout.write(
  `Completed ${results.length} cases. Mean ${Math.round(total / elapsed.length)} ms, p50 ${percentile(0.5)} ms, p95 ${percentile(0.95)} ms.\n`,
);
