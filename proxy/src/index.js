// Proxy de tradução do Verso: guarda a chave da Azure, compartilha um cache entre todos os
// usuários (música popular é traduzida uma vez só) e impõe limites para a cota grátis
// (F0, 2M caracteres/mês) nunca estourar.

const AZURE_URL = "https://api.cognitive.microsofttranslator.com/translate?api-version=3.0";

// Idiomas de destino aceitos (códigos da Azure). Fora da lista, 400: evita que o proxy
// vire um tradutor genérico.
export const TARGETS = new Set(["pt", "pt-pt", "en", "es", "fr", "de", "it", "ja", "ko", "zh-Hans"]);

export const MAX_LINES = 400;
export const MAX_LINE_CHARS = 300;
export const MAX_TOTAL_CHARS = 10_000;
const CACHE_TTL_S = 365 * 24 * 3600;

const json = (status, body) =>
  new Response(JSON.stringify(body), { status, headers: { "content-type": "application/json; charset=utf-8" } });

async function sha256(text) {
  const buf = await crypto.subtle.digest("SHA-256", new TextEncoder().encode(text));
  return [...new Uint8Array(buf)].map((b) => b.toString(16).padStart(2, "0")).join("");
}

/** Valida o corpo `{ to, lines }`. Devolve a mensagem de erro, ou null se estiver ok. */
export function invalid(body) {
  if (!body || typeof body !== "object") return "corpo inválido";
  const { to, lines } = body;
  if (!TARGETS.has(to)) return "idioma de destino não suportado";
  if (!Array.isArray(lines) || lines.length === 0) return "lines deve ser uma lista não vazia";
  if (lines.length > MAX_LINES) return `máximo de ${MAX_LINES} linhas`;
  let total = 0;
  for (const l of lines) {
    if (typeof l !== "string" || l.trim() === "") return "linhas devem ser textos não vazios";
    if (l.length > MAX_LINE_CHARS) return `linha com mais de ${MAX_LINE_CHARS} caracteres`;
    total += l.length;
  }
  if (total > MAX_TOTAL_CHARS) return `máximo de ${MAX_TOTAL_CHARS} caracteres`;
  return null;
}

/** Idioma de origem mais frequente entre as linhas (letras às vezes misturam idiomas). */
export function dominant(langs) {
  const count = new Map();
  for (const l of langs) if (l) count.set(l, (count.get(l) ?? 0) + 1);
  let best = "";
  for (const [l, n] of count) if (n > (count.get(best) ?? 0)) best = l;
  return best;
}

async function bump(kv, key, by, ttl) {
  const n = Number((await kv.get(key)) ?? 0) + by;
  await kv.put(key, String(n), ttl ? { expirationTtl: ttl } : undefined);
}

/**
 * @param {Request} request
 * @param {{ CACHE: KVNamespace, LIMITER?: RateLimit, AZURE_KEY: string, AZURE_REGION: string,
 *           MONTHLY_BUDGET?: string, IP_DAILY_CHARS?: string }} env
 * @param {{ fetch?: typeof fetch, now?: Date }} [deps] injetáveis nos testes
 */
export async function handle(request, env, deps = {}) {
  const doFetch = deps.fetch ?? fetch;
  const now = deps.now ?? new Date();
  const url = new URL(request.url);

  if (url.pathname !== "/v1/translate") return json(404, { error: "not_found" });
  if (request.method !== "POST") return json(405, { error: "method_not_allowed" });

  const ip = request.headers.get("cf-connecting-ip") ?? "unknown";
  if (env.LIMITER && !(await env.LIMITER.limit({ key: ip })).success) {
    return json(429, { error: "rate_limited" });
  }

  let body;
  try {
    body = await request.json();
  } catch {
    return json(400, { error: "bad_request", detail: "JSON inválido" });
  }
  const problem = invalid(body);
  if (problem) return json(400, { error: "bad_request", detail: problem });
  const { to, lines } = body;

  const cacheKey = `tr:v1:${to}:${await sha256(lines.join("\n"))}`;
  const hit = await env.CACHE.get(cacheKey, "json");
  if (hit) return json(200, hit);

  // Os contadores no KV são aproximados (sem atomicidade), por isso o teto fica abaixo dos 2M.
  const chars = lines.reduce((n, l) => n + l.length, 0);
  const month = now.toISOString().slice(0, 7);
  const day = now.toISOString().slice(0, 10);
  const monthKey = `usage:${month}`;
  const ipKey = `ip:${day}:${await sha256(ip)}`;
  const budget = Number(env.MONTHLY_BUDGET ?? 1_900_000);
  const ipDaily = Number(env.IP_DAILY_CHARS ?? 60_000);
  const [used, ipUsed] = await Promise.all([env.CACHE.get(monthKey), env.CACHE.get(ipKey)]);
  if (Number(used ?? 0) + chars > budget) return json(503, { error: "quota_exceeded" });
  if (Number(ipUsed ?? 0) + chars > ipDaily) return json(429, { error: "rate_limited" });

  const resp = await doFetch(`${AZURE_URL}&to=${encodeURIComponent(to)}`, {
    method: "POST",
    headers: {
      "Ocp-Apim-Subscription-Key": env.AZURE_KEY,
      "Ocp-Apim-Subscription-Region": env.AZURE_REGION,
      "Content-Type": "application/json; charset=UTF-8",
    },
    body: JSON.stringify(lines.map((Text) => ({ Text }))),
  });
  if (!resp.ok) {
    const err = await resp.json().catch(() => ({}));
    // 403001: cota grátis da Azure esgotada no mês.
    if (err?.error?.code === 403001) return json(503, { error: "quota_exceeded" });
    if (resp.status === 429) return json(429, { error: "rate_limited" });
    console.error("azure", resp.status, JSON.stringify(err));
    return json(502, { error: "upstream" });
  }
  const items = await resp.json();
  if (!Array.isArray(items) || items.length !== lines.length) return json(502, { error: "upstream" });

  const result = {
    lines: items.map((it) => it?.translations?.[0]?.text ?? ""),
    source: dominant(items.map((it) => it?.detectedLanguage?.language)),
  };
  await Promise.all([
    env.CACHE.put(cacheKey, JSON.stringify(result), { expirationTtl: CACHE_TTL_S }),
    bump(env.CACHE, monthKey, chars, 40 * 24 * 3600),
    bump(env.CACHE, ipKey, chars, 2 * 24 * 3600),
  ]);
  return json(200, result);
}

export default {
  fetch: (request, env) => handle(request, env),
};
