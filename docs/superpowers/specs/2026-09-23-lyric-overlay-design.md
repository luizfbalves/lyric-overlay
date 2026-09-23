# Lyric Overlay — Design

Data: 2026-09-23
Status: aguardando revisão

## Objetivo

App desktop pessoal (macOS e Windows) que mostra, num overlay flutuante, **somente a linha atual** da letra da música tocando no Spotify, sincronizada com a reprodução. O usuário trabalha normalmente e lê a letra sem que ela ocupe espaço ou bloqueie cliques.

**Critério de sucesso:** com o Spotify tocando uma faixa que tem letra sincronizada no LRCLIB, a linha exibida acompanha a música com atraso perceptível < ~300 ms, sem interferir no uso do resto da tela.

### Premissas

- Uso pessoal. Sem assinatura/notarização; o usuário libera Gatekeeper/SmartScreen manualmente.
- A API do Spotify não fornece letras. A fonte é o **LRCLIB** (`https://lrclib.net/api`), gratuito e sem chave.
- Faixa e posição são lidas localmente, sem OAuth do Spotify.

### Fora de escopo

- Letras não sincronizadas (texto puro), karaokê palavra por palavra.
- Tradutores além do DeepL (Claude API fica para uma versão paga futura; a trait `Translator` já deixa o encaixe pronto).
- Outros players além do Spotify.
- Configurações além de aparência, posição e offset (ex.: tamanho da fonte, número de linhas visíveis, atalhos customizáveis).
- Instalador, auto-update, assinatura de código.

## Stack

- **Tauri 2** (backend em Rust, frontend em HTML/CSS/TS sem framework).
- Crates: `tauri`, `tauri-plugin-global-shortcut`, `reqwest` (rustls), `serde`/`serde_json`, `tokio`, `keyring` (chave DeepL no Keychain/Credential Manager), `windows` (somente Windows, para SMTC).

## Arquitetura

Backend Rust com quatro módulos independentes, mais o frontend.

### `player`: leitura do Spotify

```rust
pub struct NowPlaying {
    pub title: String,
    pub artist: String,
    pub album: String,
    pub duration_ms: u64,
    pub position_ms: u64,
    pub is_playing: bool,
}

pub trait Player: Send + Sync {
    fn now_playing(&self) -> Result<Option<NowPlaying>, PlayerError>;
}
```

- **macOS (`MacSpotifyPlayer`):** executa `osascript` com um AppleScript que retorna nome, artista, álbum, duração, `player position` e `player state` do Spotify, separados por um delimitador. Se o Spotify não estiver rodando, retorna `Ok(None)` sem abrir o app (verifica `application "Spotify" is running` antes).
- **Windows (`WinSmtcPlayer`):** `GlobalSystemMediaTransportControlsSessionManager`, filtra a sessão cujo `SourceAppUserModelId` contém `Spotify`, lê `TryGetMediaPropertiesAsync` (título/artista/álbum) e `GetTimelineProperties` (posição/fim) e `GetPlaybackInfo` (status). Posição SMTC é corrigida com `LastUpdatedTime` quando está tocando.
- `Ok(None)` = nada tocando / Spotify fechado. `Err` = falha inesperada (logada).

### `lyrics`: busca e parse

- `fetch(track: &NowPlaying) -> Result<Option<Lyrics>, LyricsError>`
  1. `GET /api/get?artist_name=&track_name=&album_name=&duration=` (duração em segundos).
  2. Se 404 ou `syncedLyrics` nulo: `GET /api/search?artist_name=&track_name=` e pega o primeiro resultado com `syncedLyrics` e duração a ±3 s da faixa.
  3. Nada encontrado: `Ok(None)`.
- Timeout de 5 s, `User-Agent` identificando o app.
- `parse_lrc(&str) -> Lyrics` onde `Lyrics { lines: Vec<Line> }`, `Line { time_ms: u64, text: String }`, ordenado por tempo.
  - Suporta `[mm:ss.xx]`, `[mm:ss.xxx]`, `[mm:ss]`, múltiplas tags na mesma linha, ignora tags de metadado (`[ar:]`, `[ti:]`, `[offset:]` etc.), mantém linhas vazias (servem para limpar o overlay em pausas instrumentais).
- `Lyrics::line_at(position_ms) -> Option<usize>`: busca binária; `None` antes da primeira linha.
- **Cache em memória** por chave `(artist, title, duration_s)`: guarda `Some(Lyrics)` ou "sem letra", para não repetir buscas.

### `translate`: tradução linha a linha

```rust
pub struct Translated { pub lines: Vec<String>, pub source_lang: String }

#[async_trait]
pub trait Translator: Send + Sync {
    async fn translate(&self, lines: &[String], target: &str) -> Result<Translated, TranslateError>;
}
```

- **`DeepLTranslator`:** `POST https://api-free.deepl.com/v2/translate` com header `Authorization: DeepL-Auth-Key <chave>`, corpo `{ "text": [...], "target_lang": "PT-BR" }`. Envia só as linhas não vazias, em lotes de até 50, e remonta mantendo o índice original. Linhas vazias continuam vazias, então os tempos da letra não mudam.
- Se o `detected_source_language` for igual ao idioma-alvo (ex.: música em português com alvo PT-BR), descarta a tradução e marca a faixa como "não precisa".
- **Cache em disco** em `app_cache_dir()/translations/<hash(chave da faixa + alvo)>.json`, para nunca traduzir a mesma faixa duas vezes. Cache de faixa "não precisa" também é gravado.
- Só roda se houver chave configurada e o modo não for "Só original".
- Erros: 403 (chave inválida) → modo cai para "Só original" até a chave mudar, com aviso na janela de Preferências; 456 (cota do mês esgotada) → idem, com aviso de cota; rede/timeout (10 s) → mostra o original e tenta de novo na próxima faixa.
- A chave fica no cofre do sistema (`keyring`: Keychain no macOS, Credential Manager no Windows), **nunca** no `config.json`.
- Uso do mês: `GET /v2/usage` quando a janela de Preferências abre, exibido como "X / 500.000 caracteres".

### `sync`: loop de sincronização

- Tarefa `tokio` que consulta o `Player` a cada **1 s**.
- Entre consultas, estima a posição: `last_position + (now - last_poll_instant)` quando `is_playing`.
- Quando a letra carrega, emite `lyrics-loaded { lines: string[] }` (a letra inteira, uma vez por faixa). Quando a tradução chega (logo depois, em background), emite `translation-loaded { lines: string[] }` com o mesmo número de linhas. Até lá o overlay mostra só o original.
- Tick de render a cada **100 ms**: calcula `line_at(estimated + offset)`. Se o índice mudou, emite `line-changed { index }` (`-1` antes da primeira linha).
- A cada consulta, se `|posição real − estimada| > 1500 ms` (seek), ressincroniza imediatamente.
- Troca de faixa (chave diferente): limpa o estado, emite `hide`, dispara `lyrics::fetch` em background e só volta a exibir quando a letra chega.
- Emite `hide` quando: `Ok(None)`, `is_playing == false`, letra ausente ou erro de rede.
- **Offset por faixa:** `HashMap<chave, i64>`, persistido em config. Atalhos ajustam ±250 ms.

### Frontend (overlay)

- Janela Tauri: `transparent: true`, `decorations: false`, `alwaysOnTop: true`, `skipTaskbar: true`, `resizable: false`, `shadow: false`. Largura ~900 px, altura ~130 px (3 linhas visíveis). No macOS: `macOSPrivateApi: true` para transparência, e `visible_on_all_workspaces`.
- **Rolagem estilo letreiro:** a letra inteira é renderizada numa coluna (`track`) dentro de uma área de ~3 linhas de altura (`viewport`). A linha atual fica centralizada, opaca e em escala 1. As vizinhas ficam com ~30% de opacidade e escala ~0,85, e uma `mask-image` em gradiente faz o topo e o rodapé desaparecerem.
- Na troca de linha, o `track` desliza com `translateY` (~550 ms, easing suave) até centralizar a nova linha, e opacidade/escala fazem a transição junto. Linhas vazias (pausas instrumentais) aparecem como `• • •`.
- Fonte do sistema ~25 px, branca, com `text-shadow` forte para legibilidade em qualquer fundo. Com `prefers-reduced-motion`, a troca é instantânea.
- `hide` esconde o conteúdo (opacity 0). A janela continua existindo e com o clique atravessando.

### Interação

- **Clique atravessa** por padrão: `window.set_ignore_cursor_events(true)`.
- **`Cmd/Ctrl+Shift+L`** alterna o modo de edição:
  - Ligado: `set_ignore_cursor_events(false)`, borda tracejada visível, texto de exemplo se estiver oculto, arraste via `data-tauri-drag-region`.
  - Desligado: salva a posição da janela na config e volta o clique a atravessar.
- **`Cmd/Ctrl+Shift+←` / `→`**: offset −/+250 ms para a faixa atual. Mostra um aviso rápido "offset +250 ms" por 1 s no overlay.
- **Ícone na barra de menus (macOS) / bandeja (Windows):** é o ponto central de opções do app, pensado para receber itens futuros.
  - macOS: ícone monocromático *template* (se adapta a tema claro/escuro) na barra de menus. O app não aparece no Dock (`ActivationPolicy::Accessory`).
  - Windows: ícone na bandeja do sistema.
  - Menu inicial: título da faixa atual (desabilitado, informativo) · separador · "Mostrar/ocultar letra" · "Editar posição" · "Resetar offset desta faixa" · "Aparência…" (`Cmd+,` no macOS) · separador · "Sair".
  - O menu é montado num único módulo `tray.rs` a partir de uma lista de itens, para adicionar opções novas sem mexer no resto.

### Modo de tradução

- Três modos: **Só original**, **Só tradução**, **Original + tradução** (padrão quando há chave). No modo duplo, a **tradução é a linha principal** (tamanho cheio, em cima) e o original aparece embaixo, em ~62% do tamanho, peso menor e opacidade ~80%. A área visível do overlay cresce para ~170 px nesse modo.
- Troca pelo menu do ícone (itens de rádio com ✓, sob o título "Tradução"). Salvo na config.

### Preferências

- Janela separada "Preferências" (Tauri, com decoração normal, ~560×440, não redimensionável), aberta pelo item "Preferências…" do menu. Se já estiver aberta, só ganha foco. Tem duas seções: **Aparência** e **Tradução**.

**Aparência**

- **Fonte:** 4 predefinições, com as fontes empacotadas no app como `woff2` (licença OFL), para ficarem iguais no macOS e no Windows:
  - Sistema (`-apple-system` / `Segoe UI`, não empacotada)
  - Arredondada: Nunito
  - Serifada: Lora
  - Mono: JetBrains Mono
- **Cor do texto:** 5 amostras (branco, amarelo, verde, azul, grafite) + seletor de cor livre. Padrão: branco.
- **Fundo:** "sem fundo" (padrão) + 3 amostras (preto, azul-noite, branco) + seletor de cor livre + slider de opacidade (10–100%, padrão 60%, desabilitado quando sem fundo). O fundo é um retângulo arredondado atrás da área de 3 linhas. Com fundo, o `text-shadow` é removido; sem fundo, fica o `text-shadow` forte.
- Botão "Restaurar padrão".

**Tradução**

- Campo da chave DeepL (tipo senha; salvo no `keyring` ao sair do campo) e seletor de idioma-alvo (Português (Brasil), Inglês (EUA), Espanhol; padrão PT-BR).
- Mostra o uso do mês e avisos de chave inválida ou cota esgotada.
- Cada mudança é aplicada na hora: a janela de Aparência chama o comando `set_appearance`, o backend salva na config e emite `appearance-changed` para o overlay, que atualiza variáveis CSS (`--ov-font`, `--ov-color`, `--ov-bg`) e recentraliza a linha atual.

### Config

Arquivo JSON em `app_config_dir()/config.json`:

```json
{
  "window": { "x": 0, "y": 0 },
  "offsets": { "<artist>|<title>|<duration_s>": 250 },
  "appearance": { "font": "system", "text_color": "#ffffff", "bg_color": null, "bg_opacity": 60 },
  "translation": { "mode": "both", "target_lang": "PT-BR" }
}
```

`font` ∈ `system | rounded | serif | mono`; `mode` ∈ `original | translated | both`. Valores inválidos ou ausentes voltam ao padrão.

Posição padrão: centralizado horizontalmente, a ~120 px da borda inferior do monitor principal. Se a posição salva estiver fora de todos os monitores, volta ao padrão.

## Fluxo de dados

```
Player (1 s) ──► sync ──(faixa nova)──► lyrics.fetch ──► cache ──► translate (DeepL, cache em disco)
                   │                                       │
                   └── tick 100 ms: line_at(pos + offset) ◄┘
                                   │
                        lyrics-loaded / line-changed / hide
                                   ▼
                            Overlay (webview)
```

## Tratamento de erros

| Situação | Comportamento |
|---|---|
| Spotify fechado ou pausado | `hide`; o polling continua |
| LRCLIB sem letra sincronizada | tenta `/search`; se falhar, `hide` e cache "sem letra" |
| Rede fora ou timeout | `hide`; nova tentativa na próxima troca de faixa (não grava no cache) |
| Seek | ressincroniza quando o desvio passa de 1,5 s |
| macOS nega permissão de Automação | loga o erro, `hide`; o polling continua (funciona quando a permissão for concedida) |
| Erro inesperado do player | loga, trata como `None` |
| DeepL 403 / 456 | mostra só o original; aviso na janela de Preferências |
| DeepL rede/timeout | mostra só o original; tenta de novo na próxima faixa |

Nenhum erro é mostrado no overlay; o app nunca trava por falha externa.

## Testes

- **Unitários (Rust):**
  - `parse_lrc`: formatos de timestamp, múltiplas tags, metadados, linhas vazias, entrada inválida.
  - `line_at`: antes da primeira linha, exato, entre linhas, após a última.
  - Estimativa de posição e detecção de seek (relógio injetável).
  - Lógica do `sync` com `FakePlayer` e letra fake: troca de faixa, pausa, offset.
  - `config`: leitura com campos ausentes/inválidos cai no padrão; `appearance` faz round-trip.
- **`translate`:** mock HTTP do DeepL cobrindo lote com linhas vazias (índices preservados), mais de 50 linhas (vários lotes), idioma de origem igual ao alvo, 403, 456 e timeout; cache em disco com round-trip e hit.
- **Cliente LRCLIB:** servidor HTTP mock (`wiremock` ou `mockito`) cobrindo 200, 404 + fallback `/search`, e timeout.
- **Fixtures:** somente textos inventados; nenhuma letra real no repositório.
- **Manual:** `MacSpotifyPlayer` no macOS e `WinSmtcPlayer` no Windows; overlay com clique atravessando, modo de edição e persistência de posição em ambos.

## Estrutura de pastas

```
lyric-overlay/
├── src/                 # frontend: overlay (index.html, main.ts, style.css) e prefs.html/prefs.ts
│   └── fonts/           # Nunito, Lora, JetBrains Mono (woff2)
└── src-tauri/src/
    ├── main.rs          # setup Tauri, janela, atalhos
    ├── tray.rs          # ícone da barra de menus/bandeja + menu
    ├── config.rs        # inclui Appearance + validação
    ├── player/{mod.rs, macos.rs, windows.rs}
    ├── lyrics/{mod.rs, lrc.rs, lrclib.rs}
    ├── translate/{mod.rs, deepl.rs, cache.rs}
    └── sync.rs
```
