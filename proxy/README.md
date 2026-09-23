# Proxy de tradução

Cloudflare Worker entre o app e a Azure Translator. Ele guarda a chave da Azure, mantém um cache
compartilhado (música popular é traduzida uma vez só para todos) e segura a cota grátis F0
(2M caracteres/mês) com três limites:

- teto mensal global (`MONTHLY_BUDGET`, 1,9M: os contadores no KV são aproximados);
- caracteres novos por IP por dia (`IP_DAILY_CHARS`, 60 mil, umas 30 músicas);
- requisições por IP por minuto (rate limiter do Cloudflare, 30/min).

Com a cota esgotada, o proxy responde `503 {"error":"quota_exceeded"}` e o app mostra só a letra
original, tentando de novo a cada hora.

## API

`POST /v1/translate` com `{"to": "pt", "lines": ["...", "..."]}` responde
`{"lines": ["...", "..."], "source": "ja"}`. Linhas vazias não são aceitas: o app filtra antes.

## Deploy

1. **Azure**: no [portal](https://portal.azure.com), crie um recurso **Translator** com o tipo de preço
   **F0 (Free)**. Em *Keys and Endpoint*, copie a **Key 1** e a **Location/Region** (ex.: `brazilsouth`).
2. **Cloudflare**, dentro desta pasta:
   ```sh
   npx wrangler login
   npx wrangler kv namespace create CACHE   # copie o id para o wrangler.toml
   npx wrangler secret put AZURE_KEY        # cole a Key 1
   ```
   Se a região não for `brazilsouth`, ajuste `AZURE_REGION` no `wrangler.toml`.
3. `npm run deploy`. O comando imprime a URL, algo como `https://verso-translate.<conta>.workers.dev`.
4. Teste:
   ```sh
   curl -X POST https://verso-translate.<conta>.workers.dev/v1/translate \
     -H 'content-type: application/json' -d '{"to":"pt","lines":["Hello, world"]}'
   ```
5. Coloque a URL em `PROXY_URL`, no [src-tauri/src/translate/mod.rs](../src-tauri/src/translate/mod.rs).
   Com ela preenchida, a tradução aparece no app (menu *Tradução* e Preferências).

## Testes

```sh
npm test
```
