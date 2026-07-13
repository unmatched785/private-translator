// UI-only development server. It deliberately keeps mock records in memory and never writes them.
import http from "node:http";
import { readFile } from "node:fs/promises";
import { randomUUID } from "node:crypto";
import { fileURLToPath } from "node:url";
import path from "node:path";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const records = [];
const memories = [];
const supportedLanguages = [
  "ko", "en", "ja", "zh", "zh-Hant", "fr", "de", "es", "pt", "it", "ru", "ar", "tr", "th",
  "vi", "id", "ms", "tl", "hi", "pl", "cs", "nl", "uk", "he", "fa", "bn", "ta", "te", "mr",
  "gu", "ur", "km", "my", "bo", "kk", "mn", "ug", "yue",
];

const config = {
  profile: "demo",
  display_name: "Private Translator Demo",
  default_model: "local-demo",
  max_text_chars: 50000,
  models: [
    {
      id: "local-demo",
      label: "Local demo engine",
      description: "Development engine for testing the interface and vault",
      privacy: "device",
      available: true,
      supported_languages: supportedLanguages,
    },
    {
      id: "hy-mt2-7b-preview",
      label: "Hy-MT2 7B · Interface preview",
      description: "Preview entry for the Quality profile model selector",
      privacy: "private_network",
      available: false,
      supported_languages: supportedLanguages,
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
    if (request.method === "GET" && url.pathname === "/i18n.js") {
      return file(response, "web/i18n.js", "application/javascript; charset=utf-8");
    }
    if (request.method === "GET" && url.pathname === "/styles.css") {
      return file(response, "web/styles.css", "text/css; charset=utf-8");
    }
    if (
      url.pathname.startsWith("/api/") &&
      !/^Bearer [0-9a-f]{64}$/i.test(request.headers.authorization || "")
    ) {
      return json(response, 401, {
        error: { code: "session_required", message: "Open the mock UI with a token fragment." },
      });
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
          model_label: config.models.find((model) => model.id === body.model)?.label || "Local demo",
          mode: body.mode || "instant",
          privacy: "device",
          latency_ms: Math.max(24, Date.now() - started),
          qa_warnings: [],
          approved_memory_id: null,
          approved_revision: null,
        });
      }
      await new Promise((resolve) => setTimeout(resolve, 180));
      return json(response, 200, {
        translated_text: translatedText,
        source: body.source,
        target: body.target,
        model_id: body.model,
        model_label: config.models.find((model) => model.id === body.model)?.label || "Local demo",
        privacy: "device",
        latency_ms: 180,
        chunk_count: 1,
        history_id: historyId,
        history_error: null,
        qa_warnings: [],
      });
    }
    if (request.method === "POST" && url.pathname === "/api/history/search") {
      const body = await readJson(request);
      const query = String(body.query || "").trim().toLocaleLowerCase();
      const favorites = body.favorites === true;
      const approved = body.approved === true;
      const requestedLimit = Number.isInteger(body.limit) ? body.limit : 100;
      const limit = Math.max(1, Math.min(requestedLimit, 500));
      const filtered = records.filter((record) => {
        if (favorites && !record.favorite) return false;
        if (approved && !record.approved_memory_id) return false;
        if (!query) return true;
        return `${record.source_text}\n${record.translated_text}`.toLocaleLowerCase().includes(query);
      });
      return json(response, 200, { records: filtered.slice(0, limit) });
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
    if (request.method === "GET" && url.pathname === "/api/memory") {
      return json(response, 200, { records: memories.map(publicMemory) });
    }

    const favoriteMatch = url.pathname.match(/^\/api\/history\/([^/]+)\/favorite$/);
    if (request.method === "PATCH" && favoriteMatch) {
      const record = records.find((item) => item.id === decodeURIComponent(favoriteMatch[1]));
      if (!record) return apiError(response, 404, "history_not_found", "The translation record was not found.");
      const body = await readJson(request);
      record.favorite = Boolean(body.favorite);
      response.writeHead(204);
      return response.end();
    }

    const approveMatch = url.pathname.match(/^\/api\/history\/([^/]+)\/approve$/);
    if (request.method === "POST" && approveMatch) {
      const record = records.find((item) => item.id === decodeURIComponent(approveMatch[1]));
      if (!record) return apiError(response, 404, "history_not_found", "The translation record was not found.");
      let memory = memories.find((item) => item.history_id === record.id);
      if (!memory) {
        const now = Date.now();
        memory = {
          ...record,
          id: randomUUID(),
          history_id: record.id,
          created_at: now,
          updated_at: now,
          revision: 1,
          revision_created_at: now,
          revision_kind: "approved",
          revisions: [],
        };
        memory.revisions.push(publicMemory(memory));
        memories.unshift(memory);
        record.approved_memory_id = memory.id;
        record.approved_revision = 1;
      }
      return json(response, 200, publicMemory(memory));
    }

    const revisionsMatch = url.pathname.match(/^\/api\/memory\/([^/]+)\/revisions$/);
    if (request.method === "GET" && revisionsMatch) {
      const memory = memories.find((item) => item.id === decodeURIComponent(revisionsMatch[1]));
      if (!memory) return apiError(response, 404, "memory_not_found", "The translation asset was not found.");
      return json(response, 200, { records: [...memory.revisions].reverse() });
    }

    const memoryMatch = url.pathname.match(/^\/api\/memory\/([^/]+)$/);
    if (memoryMatch) {
      const memoryId = decodeURIComponent(memoryMatch[1]);
      const memory = memories.find((item) => item.id === memoryId);
      if (!memory) return apiError(response, 404, "memory_not_found", "The translation asset was not found.");
      if (request.method === "GET") return json(response, 200, publicMemory(memory));
      if (request.method === "PUT") {
        const body = await readJson(request);
        memory.source_text = body.source_text;
        memory.translated_text = body.translated_text;
        memory.source_lang = body.source_lang;
        memory.target_lang = body.target_lang;
        memory.revision += 1;
        memory.revision_kind = "edited";
        memory.revision_created_at = Date.now();
        memory.updated_at = memory.revision_created_at;
        memory.revisions.push(publicMemory(memory));
        const linked = records.find((item) => item.id === memory.history_id);
        if (linked) linked.approved_revision = memory.revision;
        return json(response, 200, publicMemory(memory));
      }
      if (request.method === "DELETE") {
        const linked = records.find((item) => item.id === memory.history_id);
        if (linked) {
          linked.approved_memory_id = null;
          linked.approved_revision = null;
        }
        memories.splice(
          memories.findIndex((item) => item.id === memoryId),
          1,
        );
        response.writeHead(204);
        return response.end();
      }
    }

    const historyMatch = url.pathname.match(/^\/api\/history\/([^/]+)$/);
    if (request.method === "DELETE" && historyMatch) {
      const index = records.findIndex((item) => item.id === decodeURIComponent(historyMatch[1]));
      if (index < 0) return apiError(response, 404, "history_not_found", "The translation record was not found.");
      const memory = memories.find((item) => item.history_id === records[index].id);
      if (memory) memory.history_id = null;
      records.splice(index, 1);
      response.writeHead(204);
      return response.end();
    }

    return apiError(response, 404, "route_not_found", "The requested path was not found.");
  } catch (error) {
    return apiError(response, 500, "storage_error", error.message);
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

function apiError(response, status, code, message) {
  return json(response, status, { error: { code, message } });
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
  return `[Local demo · ${label}] ${String(text).trim()}`;
}

function publicMemory(memory) {
  const { revisions, favorite, approved_memory_id, approved_revision, ...record } = memory;
  return { ...record, qa_warnings: record.qa_warnings || [] };
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
