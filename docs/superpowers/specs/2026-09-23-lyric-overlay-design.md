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

- Letras não sincronizadas (texto puro), tradução, karaokê palavra por palavra.
- Outros players além do Spotify.
- Tela de configurações. Os únicos ajustes são posição e offset, feitos via atalho ou pelo menu do ícone.
- Instalador, auto-update, assinatura de código.

## Stack

- **Tauri 2** (backend em Rust, frontend em HTML/CSS/TS sem framework).
- Crates: `tauri`, `tauri-plugin-global-shortcut`, `reqwest` (rustls), `serde`/`serde_json`, `tokio`, `windows` (somente Windows, para SMTC).

## Arquitetura

Backend Rust com três módulos independentes, mais o frontend.

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

### `sync`: loop de sincronização

- Tarefa `tokio` que consulta o `Player` a cada **1 s**.
- Entre consultas, estima a posição: `last_position + (now - last_poll_instant)` quando `is_playing`.
- Tick de render a cada **100 ms**: calcula `line_at(estimated + offset)`. Se o índice mudou, emite `line-changed { text }`. Linha vazia emite texto vazio (o frontend faz fade-out).
- A cada consulta, se `|posição real − estimada| > 1500 ms` (seek), ressincroniza imediatamente.
- Troca de faixa (chave diferente): limpa o estado, emite `hide`, dispara `lyrics::fetch` em background e só volta a exibir quando a letra chega.
- Emite `hide` quando: `Ok(None)`, `is_playing == false`, letra ausente ou erro de rede.
- **Offset por faixa:** `HashMap<chave, i64>`, persistido em config. Atalhos ajustam ±250 ms.

### Frontend (overlay)

- Janela Tauri: `transparent: true`, `decorations: false`, `alwaysOnTop: true`, `skipTaskbar: true`, `resizable: false`, `shadow: false`. Largura ~900 px, altura ~80 px. No macOS: `macOSPrivateApi: true` para transparência, e `visible_on_all_workspaces`.
- Mostra uma linha centralizada, fonte do sistema ~26 px, branca, com `text-shadow` forte para legibilidade em qualquer fundo. Textos longos quebram em no máximo 2 linhas com ellipsis.
- Transição: fade de ~150 ms na troca de linha.
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
  - Menu inicial: título da faixa atual (desabilitado, informativo) · separador · "Mostrar/ocultar letra" · "Editar posição" · "Resetar offset desta faixa" · separador · "Sair".
  - O menu é montado num único módulo `tray.rs` a partir de uma lista de itens, para adicionar opções novas sem mexer no resto.

### Config

Arquivo JSON em `app_config_dir()/config.json`:

```json
{ "window": { "x": 0, "y": 0 }, "offsets": { "<artist>|<title>|<duration_s>": 250 } }
```

Posição padrão: centralizado horizontalmente, a ~120 px da borda inferior do monitor principal. Se a posição salva estiver fora de todos os monitores, volta ao padrão.

## Fluxo de dados

```
Player (1 s) ──► sync ──(faixa nova)──► lyrics.fetch ──► cache
                   │                                       │
                   └── tick 100 ms: line_at(pos + offset) ◄┘
                                   │
                        event line-changed / hide
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

Nenhum erro é mostrado no overlay; o app nunca trava por falha externa.

## Testes

- **Unitários (Rust):**
  - `parse_lrc`: formatos de timestamp, múltiplas tags, metadados, linhas vazias, entrada inválida.
  - `line_at`: antes da primeira linha, exato, entre linhas, após a última.
  - Estimativa de posição e detecção de seek (relógio injetável).
  - Lógica do `sync` com `FakePlayer` e letra fake: troca de faixa, pausa, offset.
- **Cliente LRCLIB:** servidor HTTP mock (`wiremock` ou `mockito`) cobrindo 200, 404 + fallback `/search`, e timeout.
- **Fixtures:** somente textos inventados; nenhuma letra real no repositório.
- **Manual:** `MacSpotifyPlayer` no macOS e `WinSmtcPlayer` no Windows; overlay com clique atravessando, modo de edição e persistência de posição em ambos.

## Estrutura de pastas

```
lyric-overlay/
├── src/                 # frontend: index.html, main.ts, style.css
└── src-tauri/src/
    ├── main.rs          # setup Tauri, janela, atalhos
    ├── tray.rs          # ícone da barra de menus/bandeja + menu
    ├── config.rs
    ├── player/{mod.rs, macos.rs, windows.rs}
    ├── lyrics/{mod.rs, lrc.rs, lrclib.rs}
    └── sync.rs
```
