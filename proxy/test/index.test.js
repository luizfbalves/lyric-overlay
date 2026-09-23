import assert from "node:assert/strict";
import { test } from "node:test";
import { dominant, handle, invalid } from "../src/index.js";

class FakeKV {
  constructor() {
    this.data = new Map();
  }
  async get(k, type) {
    const v = this.data.get(k);
    if (v === undefined) return null;
    return type === "json" ? JSON.parse(v) : v;
  }
  async put(k, v) {
    this.data.set(k, v);
  }
}

/** Azure falsa: traduz para "tr:<texto>" e detecta o idioma fixo. */
function azure(lang = "ja") {
  const calls = [];
  const fn = async (url, init) => {
    calls.push({ url, init });
    const body = JSON.parse(init.body);
    return Response.json(
      body.map((b) => ({ detectedLanguage: { language: lang, score: 1 }, translations: [{ text: `tr:${b.Text}`, to: "pt" }] })),
    );
  };
  fn.calls = calls;
  return fn;
}

const env = (extra = {}) => ({ CACHE: new FakeKV(), AZURE_KEY: "k", AZURE_REGION: "brazilsouth", ...extra });

const req = (body, { path = "/v1/translate", method = "POST", ip = "1.2.3.4" } = {}) =>
  new Request(`https://proxy.test${path}`, {
    method,
    headers: { "content-type": "application/json", "cf-connecting-ip": ip },
    body: method === "POST" ? JSON.stringify(body) : undefined,
  });

test("traduz e manda chave e região para a Azure", async () => {
  const e = env();
  const f = azure();
  const r = await handle(req({ to: "pt", lines: ["a", "b"] }), e, { fetch: f });
  assert.equal(r.status, 200);
  assert.deepEqual(await r.json(), { lines: ["tr:a", "tr:b"], source: "ja" });
  assert.equal(f.calls.length, 1);
  assert.match(f.calls[0].url, /to=pt$/);
  assert.equal(f.calls[0].init.headers["Ocp-Apim-Subscription-Key"], "k");
  assert.equal(f.calls[0].init.headers["Ocp-Apim-Subscription-Region"], "brazilsouth");
});

test("segunda chamada igual vem do cache, sem Azure", async () => {
  const e = env();
  const f = azure();
  await handle(req({ to: "pt", lines: ["a"] }), e, { fetch: f });
  const r = await handle(req({ to: "pt", lines: ["a"] }, { ip: "9.9.9.9" }), e, { fetch: f });
  assert.equal(r.status, 200);
  assert.deepEqual(await r.json(), { lines: ["tr:a"], source: "ja" });
  assert.equal(f.calls.length, 1);
});

test("cache separa idiomas de destino", async () => {
  const e = env();
  const f = azure();
  await handle(req({ to: "pt", lines: ["a"] }), e, { fetch: f });
  await handle(req({ to: "en", lines: ["a"] }), e, { fetch: f });
  assert.equal(f.calls.length, 2);
});

test("teto mensal devolve 503 quota_exceeded sem chamar a Azure", async () => {
  const e = env({ MONTHLY_BUDGET: "5" });
  const f = azure();
  const now = new Date("2026-09-10T12:00:00Z");
  assert.equal((await handle(req({ to: "pt", lines: ["abc"] }), e, { fetch: f, now })).status, 200);
  const r = await handle(req({ to: "pt", lines: ["xyz"] }), e, { fetch: f, now });
  assert.equal(r.status, 503);
  assert.deepEqual(await r.json(), { error: "quota_exceeded" });
  assert.equal(f.calls.length, 1);
  // Mês novo, contador novo.
  const next = await handle(req({ to: "pt", lines: ["xyz"] }), e, { fetch: f, now: new Date("2026-10-01T00:00:00Z") });
  assert.equal(next.status, 200);
});

test("limite diário por IP devolve 429, outro IP segue", async () => {
  const e = env({ IP_DAILY_CHARS: "5" });
  const f = azure();
  assert.equal((await handle(req({ to: "pt", lines: ["abc"] }), e, { fetch: f })).status, 200);
  assert.equal((await handle(req({ to: "pt", lines: ["xyz"] }), e, { fetch: f })).status, 429);
  assert.equal((await handle(req({ to: "pt", lines: ["xyz"] }, { ip: "5.6.7.8" }), e, { fetch: f })).status, 200);
});

test("rate limiter do Cloudflare bloqueia antes de tudo", async () => {
  const e = env({ LIMITER: { limit: async () => ({ success: false }) } });
  const f = azure();
  assert.equal((await handle(req({ to: "pt", lines: ["a"] }), e, { fetch: f })).status, 429);
  assert.equal(f.calls.length, 0);
});

test("cota da Azure esgotada (403001) vira 503 quota_exceeded", async () => {
  const f = async () => Response.json({ error: { code: 403001, message: "free quota" } }, { status: 403 });
  const r = await handle(req({ to: "pt", lines: ["a"] }), env(), { fetch: f });
  assert.equal(r.status, 503);
  assert.deepEqual(await r.json(), { error: "quota_exceeded" });
});

test("outros erros da Azure viram 502 e não entram no cache", async () => {
  const e = env();
  const bad = async () => Response.json({ error: { code: 401000 } }, { status: 401 });
  assert.equal((await handle(req({ to: "pt", lines: ["a"] }), e, { fetch: bad })).status, 502);
  const f = azure();
  assert.equal((await handle(req({ to: "pt", lines: ["a"] }), e, { fetch: f })).status, 200);
  assert.equal(f.calls.length, 1);
});

test("rotas e métodos errados", async () => {
  assert.equal((await handle(req({}, { path: "/" }), env())).status, 404);
  assert.equal((await handle(req(null, { method: "GET" }), env())).status, 405);
});

test("validação da entrada", () => {
  assert.equal(invalid({ to: "pt", lines: ["ok"] }), null);
  assert.ok(invalid({ to: "xx", lines: ["ok"] }));
  assert.ok(invalid({ to: "pt", lines: [] }));
  assert.ok(invalid({ to: "pt", lines: [""] }));
  assert.ok(invalid({ to: "pt", lines: [42] }));
  assert.ok(invalid({ to: "pt", lines: ["x".repeat(301)] }));
  assert.ok(invalid({ to: "pt", lines: Array(401).fill("x") }));
  assert.ok(invalid({ to: "pt", lines: Array(40).fill("x".repeat(300)) }));
  assert.ok(invalid(null));
});

test("idioma de origem é o mais frequente", () => {
  assert.equal(dominant(["en", "ja", "ja", undefined]), "ja");
  assert.equal(dominant([]), "");
});
