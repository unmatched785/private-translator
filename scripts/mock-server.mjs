// UI-only development server. It deliberately keeps mock records in memory and never writes them.
import http from "node:http";
import { readFile } from "node:fs/promises";
import { randomUUID } from "node:crypto";
import { fileURLToPath } from "node:url";
import path from "node:path";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const records = [];

const config = {
  profile: "demo",
  display_name: "Private Translator Demo",
  default_model: "local-demo",
  max_text_chars: 50000,
  models: [
    {
      id: "local-demo",
      label: "로컬 데모 엔진",
      description: "화면과 기록 동작을 확인하는 개발용 엔진",
      privacy: "device",
    },
    {
      id: "hy-mt2-7b-preview",
      label: "Hy-MT2 7B · 선택 화면 미리보기",
      description: "Quality 프로필의 모델 선택 화면을 확인하는 항목",
      privacy: "private_network",
    },
  ],
};

const server = http.createServer(async (request, response) => {
  try {
    const url = new URL(request.url, "http://127.0.0.1:8173");
    setSecurityHeaders(response);

    if (request.method === "GET" && url.pathname === "/") {
      return file(response, "web/index.html", "text/html; charset=utf-8");
    }
    if (request.method === "GET" && url.pathname === "/app.js") {
      return file(response, "web/app.js", "application/javascript; charset=utf-8");
    }
    if (request.method === "GET" && url.pathname === "/styles.css") {
      return file(response, "web/styles.css", "text/css; charset=utf-8");
    }
    if (request.method === "GET" && url.pathname === "/api/config") {
      return json(response, 200, config);
    }
    if (request.method === "GET" && url.pathname === "/api/health") {
      return json(response, 200, {
        status: "ok",
        profile: "demo",
        vault: "ui-test-memory-only",
        history_count: records.length,
      });
    }
    if (request.method === "POST" && url.pathname === "/api/translate") {
      const body = await readJson(request);
      const started = Date.now();
      const translatedText = mockTranslate(body.text, body.target);
      let historyId = null;
      if (body.save_history !== false) {
        historyId = randomUUID();
        records.unshift({
          id: historyId,
          created_at: Date.now(),
          favorite: false,
          source_text: body.text,
          translated_text: translatedText,
          source_lang: body.source || "auto",
          target_lang: body.target,
          model_id: body.model,
          model_label: config.models.find((model) => model.id === body.model)?.label || "로컬 데모",
          mode: body.mode || "instant",
          privacy: "device",
          latency_ms: Math.max(24, Date.now() - started),
          qa_warnings: [],
        });
      }
      await new Promise((resolve) => setTimeout(resolve, 180));
      return json(response, 200, {
        translated_text: translatedText,
        source: body.source,
        target: body.target,
        model_id: body.model,
        model_label: config.models.find((model) => model.id === body.model)?.label || "로컬 데모",
        privacy: "device",
        latency_ms: 180,
        chunk_count: 1,
        history_id: historyId,
        qa_warnings: [],
      });
    }
    if (request.method === "GET" && url.pathname === "/api/history") {
      const query = (url.searchParams.get("q") || "").toLocaleLowerCase();
      const favorites = url.searchParams.get("favorites") === "true";
      const filtered = records.filter((record) => {
        if (favorites && !record.favorite) return false;
        if (!query) return true;
        return `${record.source_text}\n${record.translated_text}`.toLocaleLowerCase().includes(query);
      });
      return json(response, 200, { records: filtered });
    }

    const favoriteMatch = url.pathname.match(/^\/api\/history\/([^/]+)\/favorite$/);
    if (request.method === "PATCH" && favoriteMatch) {
      const record = records.find((item) => item.id === decodeURIComponent(favoriteMatch[1]));
      if (!record) return json(response, 404, { error: { message: "기록을 찾을 수 없습니다" } });
      const body = await readJson(request);
      record.favorite = Boolean(body.favorite);
      response.writeHead(204);
      return response.end();
    }

    const historyMatch = url.pathname.match(/^\/api\/history\/([^/]+)$/);
    if (request.method === "DELETE" && historyMatch) {
      const index = records.findIndex((item) => item.id === decodeURIComponent(historyMatch[1]));
      if (index < 0) return json(response, 404, { error: { message: "기록을 찾을 수 없습니다" } });
      records.splice(index, 1);
      response.writeHead(204);
      return response.end();
    }

    return json(response, 404, { error: { message: "경로를 찾을 수 없습니다" } });
  } catch (error) {
    return json(response, 500, { error: { message: error.message } });
  }
});

server.listen(8173, "127.0.0.1", () => {
  console.log("UI-only mock server: http://127.0.0.1:8173");
  console.log("Mock history is memory-only and disappears when this process stops.");
});

async function file(response, relativePath, contentType) {
  response.setHeader("content-type", contentType);
  response.writeHead(200);
  response.end(await readFile(path.join(root, relativePath)));
}

function json(response, status, body) {
  response.setHeader("content-type", "application/json; charset=utf-8");
  response.writeHead(status);
  response.end(JSON.stringify(body));
}

async function readJson(request) {
  const chunks = [];
  for await (const chunk of request) chunks.push(chunk);
  return JSON.parse(Buffer.concat(chunks).toString("utf8") || "{}");
}

function mockTranslate(text, target) {
  const normalized = String(text).trim().toLocaleLowerCase();
  if (target === "ko" && normalized === "hello") return "안녕하세요!";
  if (target === "ko" && normalized === "thank you") return "감사합니다.";
  if (target === "en" && normalized === "안녕하세요") return "Hello.";
  const label = new Map([
    ["ko", "한국어"],
    ["en", "English"],
    ["ja", "日本語"],
  ]).get(target) || target;
  return `[로컬 데모 · ${label}] ${String(text).trim()}`;
}

function setSecurityHeaders(response) {
  response.setHeader("cache-control", "no-store");
  response.setHeader("x-content-type-options", "nosniff");
  response.setHeader("referrer-policy", "no-referrer");
  response.setHeader(
    "content-security-policy",
    "default-src 'self'; connect-src 'self'; img-src 'self' data:; style-src 'self'; script-src 'self'; frame-ancestors 'none'; base-uri 'none'; form-action 'self'",
  );
}
