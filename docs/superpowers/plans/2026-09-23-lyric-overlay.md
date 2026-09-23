# Lyric Overlay Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** App desktop pessoal (macOS/Windows) que lê a faixa do Spotify, busca a letra sincronizada no LRCLIB, traduz pelo DeepL e mostra num overlay transparente que rola a letra com foco na linha atual.

**Architecture:** Backend Tauri 2 em Rust com módulos puros e testáveis (`config`, `lyrics`, `player`, `translate`, `sync::Engine`, `geometry`) e uma camada fina de integração (`sync::runtime`, `sink`, `commands`, `overlay`, `tray`, `shortcuts`). O `Engine` é uma máquina de estados pura (recebe `Instant` por parâmetro); o `runtime` faz o I/O e fala com o frontend por eventos. Frontend em TS puro com Vite multipágina (`index.html` = overlay, `prefs.html` = Preferências).

**Tech Stack:** Tauri 2, Rust (tokio, reqwest 0.12/rustls, serde, async-trait, keyring 3, windows 0.58), TypeScript + Vite, @fontsource (woff2), wiremock + tempfile (testes).

**Spec:** `docs/superpowers/specs/2026-09-23-lyric-overlay-design.md`

### Desvios conscientes da spec (menores, só de organização)

- Setup do Tauri fica em `lib.rs` (`run()`), com `main.rs` só chamando `run()` — convenção do Tauri 2, necessária para `cargo test --lib`.
- `sync.rs` vira `sync/mod.rs` (Engine puro) + `sync/runtime.rs` (loop com I/O).
- Módulos extras de integração: `geometry.rs`, `state.rs`, `sink.rs`, `commands.rs`, `overlay.rs`, `prefs.rs`, `shortcuts.rs`, `secrets.rs`, `links.rs`, `icon.rs`, `translate/service.rs`.
- Fontes empacotadas via pacotes `@fontsource/*` (woff2 copiados pelo Vite para `dist/`) em vez de arquivos soltos em `src/fonts/`.
- Eventos extras além dos da spec: `show`, `mode-changed`, `offset-changed`, `edit-mode`, `deepl-status`.

## Global Constraints

- Tauri **2.x** (`tauri = "2"`, `@tauri-apps/cli@^2`, `@tauri-apps/api@^2`). Não usar Tauri 3 alpha.
- Crates fixas: `reqwest = 0.12` (`default-features = false`, `json`, `rustls-tls`), `keyring = 3` (`apple-native`, `windows-native`), `windows = 0.58` (só Windows), `async-trait = 0.1`, `tokio = 1`. Dev: `wiremock = 0.6`, `tempfile = 3`. Não adicionar outras dependências sem perguntar.
- **Fixtures somente com texto inventado.** Nenhuma letra real em testes, exemplos, commits ou textos de amostra.
- Letras sempre inseridas no DOM via `textContent`, nunca `innerHTML`.
- A chave DeepL nunca vai para `config.json`, logs ou eventos.
- URLs exatas: `SUPPORT_URL = "https://buymeacoffee.com/luizfbalves"`, `https://www.deepl.com/pro-api`, `https://www.deepl.com/your-account/keys`, `https://lrclib.net`, `https://api-free.deepl.com`.
- Timeouts: LRCLIB 5 s, DeepL 10 s. Poll do player 1 s, tick 100 ms, seek > 1500 ms, passo de offset 250 ms.
- Padrões: fonte `system`, tamanho `1.0`, texto `#ffffff`, fundo `null`, opacidade `60`, modo `both`, idioma `PT-BR`.
- Janela: base 900×130 px lógicos (170 no modo `both`), largura máx. 90% do monitor, 120 px acima da borda inferior.
- Erros nunca aparecem no overlay; o app nunca trava por falha externa (log com `eprintln!`).
- Textos de UI em português; identificadores em inglês.
- Commits no formato `tipo: descrição` em português, terminando com a linha `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`.
- Comando de teste Rust: `cargo test --manifest-path src-tauri/Cargo.toml` (rodar a partir da raiz do repo).

## Review Focus

1. **Locale com vírgula decimal (pt-BR) no macOS:** `player position` volta como `"12,5"` → deve virar 12500 ms, não erro. Teste em Task 5.
2. **Letra que chega depois da troca de faixa:** um `fetch` lento da faixa A que termina quando já toca a faixa B deve ser ignorado. Teste em Task 10.
3. **Mesma faixa repetida (loop/replay):** posição salta do fim para ~0 com a mesma chave → ressincroniza sem buscar a letra de novo. Teste em Task 10.
4. **Anúncios/podcasts do Spotify:** duração 0 ou artista/título vazio → tratar como "nada tocando" (esconde, não consulta o LRCLIB). Teste em Task 10.
5. **DeepL devolve número de traduções diferente do enviado:** deve dar erro (mostra só o original), nunca desalinhar linhas. Teste em Task 7.

---

## Estrutura de arquivos

```
lyric-overlay/
├── package.json, tsconfig.json, vite.config.ts, .gitignore
├── src/
│   ├── index.html                # overlay
│   ├── prefs.html                # Preferências
│   ├── overlay/{main.ts, style.css}
│   ├── prefs/{main.ts, style.css}
│   ├── shared/{types.ts, fonts.ts}
│   └── assets/bmc/               # já existe (brand kit)
└── src-tauri/
    ├── Cargo.toml, build.rs, tauri.conf.json, Info.plist
    ├── capabilities/default.json
    ├── icons/                    # gerados pelo tauri init + bmc-menu.png
    └── src/
        ├── main.rs, lib.rs
        ├── config.rs             # Config/Appearance/Mode + load/save tolerante
        ├── geometry.rs           # cálculos puros de tamanho/posição
        ├── icon.rs               # ícone template da barra de menus (RGBA)
        ├── links.rs              # URLs + abrir no navegador
        ├── secrets.rs            # chave DeepL no keyring
        ├── state.rs              # AppState + Snapshot
        ├── sink.rs               # TauriSink (eventos → webview/tray/config)
        ├── commands.rs           # comandos invocados pelo frontend
        ├── overlay.rs            # janela do overlay (geometria, edição, visibilidade)
        ├── prefs.rs              # abrir janela de Preferências
        ├── shortcuts.rs          # atalhos globais
        ├── tray.rs               # ícone + menu
        ├── player/{mod.rs, macos.rs, windows.rs}
        ├── lyrics/{mod.rs, lrc.rs, lrclib.rs}
        ├── translate/{mod.rs, deepl.rs, cache.rs, service.rs}
        └── sync/{mod.rs, runtime.rs}
```

---

### Task 1: Scaffold do projeto Tauri 2 + Vite

**Files:**
- Create: `package.json`, `tsconfig.json`, `vite.config.ts`, `.gitignore`, `src/index.html`, `src/prefs.html`, `src/overlay/main.ts`, `src/prefs/main.ts`
- Create (via `tauri init`, depois sobrescritos): `src-tauri/Cargo.toml`, `src-tauri/build.rs`, `src-tauri/tauri.conf.json`, `src-tauri/capabilities/default.json`, `src-tauri/src/main.rs`, `src-tauri/src/lib.rs`, `src-tauri/icons/*`

**Interfaces:**
- Produces: crate lib `lyric_overlay_lib` com `pub fn run()`; `npm run build` gera `dist/index.html` e `dist/prefs.html`.

- [ ] **Step 1: Criar package.json e instalar dependências**

```bash
cd /Users/developerrhaimes/lyric-overlay
cat > package.json <<'EOF'
{
  "name": "lyric-overlay",
  "private": true,
  "version": "0.1.0",
  "type": "module",
  "scripts": {
    "dev": "vite",
    "build": "tsc --noEmit && vite build",
    "tauri": "tauri"
  }
}
EOF
npm install @tauri-apps/api@^2 @fontsource/nunito @fontsource/lora @fontsource/jetbrains-mono @fontsource/caveat @fontsource/yomogi
npm install -D @tauri-apps/cli@^2 typescript vite
```

- [ ] **Step 2: Configs de TS, Vite e .gitignore**

`tsconfig.json`:
```json
{
  "compilerOptions": {
    "target": "ES2021",
    "module": "ESNext",
    "moduleResolution": "bundler",
    "lib": ["ES2021", "DOM", "DOM.Iterable"],
    "strict": true,
    "noEmit": true,
    "isolatedModules": true,
    "skipLibCheck": true,
    "types": ["vite/client"]
  },
  "include": ["src"]
}
```

`vite.config.ts`:
```ts
import { defineConfig } from "vite";
import { fileURLToPath } from "node:url";

const src = fileURLToPath(new URL("./src", import.meta.url));

export default defineConfig({
  root: src,
  clearScreen: false,
  server: { port: 1420, strictPort: true },
  build: {
    outDir: "../dist",
    emptyOutDir: true,
    target: "es2021",
    rollupOptions: {
      input: { main: `${src}/index.html`, prefs: `${src}/prefs.html` },
    },
  },
});
```

`.gitignore`:
```
node_modules/
dist/
src-tauri/target/
src-tauri/gen/
.DS_Store
```

- [ ] **Step 3: Páginas provisórias**

`src/index.html`:
```html
<!doctype html>
<html lang="pt-BR">
  <head><meta charset="UTF-8" /><title>Lyric Overlay</title></head>
  <body><script type="module" src="./overlay/main.ts"></script></body>
</html>
```

`src/prefs.html`:
```html
<!doctype html>
<html lang="pt-BR">
  <head><meta charset="UTF-8" /><title>Preferências</title></head>
  <body><script type="module" src="./prefs/main.ts"></script></body>
</html>
```

`src/overlay/main.ts` e `src/prefs/main.ts` (provisórios, substituídos nas Tasks 15/16):
```ts
export {};
```

- [ ] **Step 4: Gerar src-tauri com o CLI**

```bash
npx tauri init --ci \
  --app-name "Lyric Overlay" --window-title "Lyric Overlay" \
  --frontend-dist ../dist --dev-url http://localhost:1420 \
  --before-dev-command "npm run dev" --before-build-command "npm run build"
ls src-tauri/icons
```
Expected: `src-tauri/` criado, com `icons/32x32.png`, `icons/icon.icns`, `icons/icon.ico` etc.

- [ ] **Step 5: Sobrescrever Cargo.toml**

`src-tauri/Cargo.toml`:
```toml
[package]
name = "lyric-overlay"
version = "0.1.0"
edition = "2021"

[lib]
name = "lyric_overlay_lib"
crate-type = ["staticlib", "cdylib", "rlib"]

[build-dependencies]
tauri-build = { version = "2", features = [] }

[dependencies]
tauri = { version = "2", features = ["tray-icon", "image-png", "macos-private-api"] }
tauri-plugin-global-shortcut = "2"
tauri-plugin-opener = "2"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
tokio = { version = "1", features = ["rt-multi-thread", "macros", "time", "sync"] }
reqwest = { version = "0.12", default-features = false, features = ["json", "rustls-tls"] }
async-trait = "0.1"
keyring = { version = "3", features = ["apple-native", "windows-native"] }

[target.'cfg(windows)'.dependencies]
windows = { version = "0.58", features = ["Media_Control", "Foundation", "Foundation_Collections"] }

[dev-dependencies]
wiremock = "0.6"
tempfile = "3"
```

`src-tauri/build.rs` (manter o gerado; conteúdo esperado):
```rust
fn main() {
    tauri_build::build()
}
```

- [ ] **Step 6: Sobrescrever tauri.conf.json e capabilities**

`src-tauri/tauri.conf.json`:
```json
{
  "$schema": "https://schema.tauri.app/config/2",
  "productName": "Lyric Overlay",
  "version": "0.1.0",
  "identifier": "dev.luizfbalves.lyricoverlay",
  "build": {
    "frontendDist": "../dist",
    "devUrl": "http://localhost:1420",
    "beforeDevCommand": "npm run dev",
    "beforeBuildCommand": "npm run build"
  },
  "app": {
    "macOSPrivateApi": true,
    "windows": [
      {
        "label": "overlay",
        "url": "index.html",
        "title": "Lyric Overlay",
        "width": 900,
        "height": 170,
        "transparent": true,
        "decorations": false,
        "alwaysOnTop": true,
        "skipTaskbar": true,
        "resizable": false,
        "shadow": false,
        "focus": false,
        "visible": false,
        "visibleOnAllWorkspaces": true
      }
    ],
    "security": { "csp": null }
  },
  "bundle": {
    "active": true,
    "targets": "all",
    "icon": ["icons/32x32.png", "icons/128x128.png", "icons/128x128@2x.png", "icons/icon.icns", "icons/icon.ico"]
  }
}
```

`src-tauri/capabilities/default.json`:
```json
{
  "$schema": "../gen/schemas/desktop-schema.json",
  "identifier": "default",
  "windows": ["overlay", "prefs"],
  "permissions": ["core:default", "core:window:allow-start-dragging"]
}
```

- [ ] **Step 7: main.rs e lib.rs mínimos**

`src-tauri/src/main.rs`:
```rust
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    lyric_overlay_lib::run()
}
```

`src-tauri/src/lib.rs`:
```rust
pub fn run() {
    tauri::Builder::default()
        .run(tauri::generate_context!())
        .expect("erro ao iniciar o Lyric Overlay");
}
```

- [ ] **Step 8: Verificar build**

Run: `npm run build && cargo check --manifest-path src-tauri/Cargo.toml`
Expected: `dist/index.html` e `dist/prefs.html` gerados; `cargo check` termina sem erros.

- [ ] **Step 9: Commit**

```bash
git add -A
git commit -m "chore: scaffold Tauri 2 + Vite multipágina

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 2: `config` — tipos e load/save tolerante

**Files:**
- Create: `src-tauri/src/config.rs`
- Modify: `src-tauri/src/lib.rs` (adicionar `pub mod config;`)

**Interfaces:**
- Produces:
  - `enum FontId { System, Rounded, Serif, Mono, Handwritten }` (serde lowercase)
  - `enum Mode { Original, Translated, Both }` (serde lowercase, padrão `Both`)
  - `enum TargetLang { PtBr, EnUs, Es }` (serde `"PT-BR" | "EN-US" | "ES"`), `fn code(&self) -> &'static str`
  - `struct Appearance { font: FontId, size: f64, text_color: String, bg_color: Option<String>, bg_opacity: u8 }` + `fn normalized(self) -> Self`
  - `struct TranslationCfg { mode: Mode, target_lang: TargetLang }`
  - `struct WindowPos { x: i32, y: i32 }`
  - `struct Config { window: Option<WindowPos>, offsets: HashMap<String, i64>, appearance: Appearance, translation: TranslationCfg }`
  - `fn load(path: &Path) -> Config`, `fn save(path: &Path, cfg: &Config) -> std::io::Result<()>`
  - `const SIZES: [f64; 4]`

- [ ] **Step 1: Escrever os testes**

`src-tauri/src/config.rs` (só o módulo de testes por enquanto, no fim do arquivo):
```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn tmp() -> tempfile::TempDir {
        tempfile::tempdir().unwrap()
    }

    #[test]
    fn missing_file_gives_defaults() {
        let d = tmp();
        let c = load(&d.path().join("config.json"));
        assert_eq!(c, Config::default());
        assert_eq!(c.appearance.size, 1.0);
        assert_eq!(c.appearance.bg_opacity, 60);
        assert_eq!(c.translation.mode, Mode::Both);
        assert_eq!(c.translation.target_lang, TargetLang::PtBr);
    }

    #[test]
    fn corrupt_file_gives_defaults() {
        let d = tmp();
        let p = d.path().join("config.json");
        std::fs::write(&p, "{ isto não é json").unwrap();
        assert_eq!(load(&p), Config::default());
    }

    #[test]
    fn partial_file_keeps_known_fields() {
        let d = tmp();
        let p = d.path().join("config.json");
        std::fs::write(&p, r#"{"appearance":{"font":"serif"}}"#).unwrap();
        let c = load(&p);
        assert_eq!(c.appearance.font, FontId::Serif);
        assert_eq!(c.appearance.text_color, "#ffffff");
        assert_eq!(c.translation, TranslationCfg::default());
    }

    #[test]
    fn invalid_values_fall_back_to_defaults() {
        let d = tmp();
        let p = d.path().join("config.json");
        std::fs::write(
            &p,
            r##"{
              "window": "no meio",
              "offsets": {"a|b|1": "muito"},
              "appearance": {"font":"comic","size":3.0,"text_color":"red","bg_color":"#12","bg_opacity":5},
              "translation": {"mode":"x","target_lang":"FR"}
            }"##,
        )
        .unwrap();
        let c = load(&p);
        assert_eq!(c.window, None);
        assert!(c.offsets.is_empty());
        assert_eq!(c.appearance, Appearance::default());
        assert_eq!(c.translation, TranslationCfg::default());
    }

    #[test]
    fn round_trip() {
        let d = tmp();
        let p = d.path().join("sub/config.json");
        let mut c = Config::default();
        c.window = Some(WindowPos { x: -300, y: 40 });
        c.offsets.insert("Banda Fictícia|Canção Teste|180".into(), 250);
        c.appearance = Appearance {
            font: FontId::Handwritten,
            size: 1.25,
            text_color: "#ffe066".into(),
            bg_color: Some("#0b1d3a".into()),
            bg_opacity: 80,
        };
        c.translation = TranslationCfg { mode: Mode::Translated, target_lang: TargetLang::Es };
        save(&p, &c).unwrap();
        assert_eq!(load(&p), c);
    }

    #[test]
    fn serialized_names_match_spec() {
        let v = serde_json::to_value(Config::default()).unwrap();
        assert_eq!(v["appearance"]["font"], "system");
        assert_eq!(v["translation"]["mode"], "both");
        assert_eq!(v["translation"]["target_lang"], "PT-BR");
        assert!(v["appearance"]["bg_color"].is_null());
        assert_eq!(TargetLang::EnUs.code(), "EN-US");
    }
}
```

- [ ] **Step 2: Rodar e ver falhar**

Adicionar `pub mod config;` em `lib.rs` (topo do arquivo).
Run: `cargo test --manifest-path src-tauri/Cargo.toml config::`
Expected: FAIL de compilação (`cannot find function load`, `Config` etc.).

- [ ] **Step 3: Implementar**

Topo de `src-tauri/src/config.rs`:
```rust
use serde::de::DeserializeOwned;
use serde::{Deserialize, Deserializer, Serialize};
use std::collections::HashMap;
use std::path::Path;

pub const SIZES: [f64; 4] = [0.8, 1.0, 1.25, 1.5];

/// Aceita qualquer JSON e cai no `Default` do tipo se não bater.
fn lenient<'de, D, T>(d: D) -> Result<T, D::Error>
where
    D: Deserializer<'de>,
    T: DeserializeOwned + Default,
{
    let v = serde_json::Value::deserialize(d)?;
    Ok(T::deserialize(v).unwrap_or_default())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum FontId {
    #[default]
    System,
    Rounded,
    Serif,
    Mono,
    Handwritten,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    Original,
    Translated,
    #[default]
    Both,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum TargetLang {
    #[default]
    #[serde(rename = "PT-BR")]
    PtBr,
    #[serde(rename = "EN-US")]
    EnUs,
    #[serde(rename = "ES")]
    Es,
}

impl TargetLang {
    pub fn code(&self) -> &'static str {
        match self {
            TargetLang::PtBr => "PT-BR",
            TargetLang::EnUs => "EN-US",
            TargetLang::Es => "ES",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Appearance {
    #[serde(deserialize_with = "lenient")]
    pub font: FontId,
    #[serde(deserialize_with = "lenient")]
    pub size: f64,
    #[serde(deserialize_with = "lenient")]
    pub text_color: String,
    #[serde(deserialize_with = "lenient")]
    pub bg_color: Option<String>,
    #[serde(deserialize_with = "lenient")]
    pub bg_opacity: u8,
}

impl Default for Appearance {
    fn default() -> Self {
        Self {
            font: FontId::System,
            size: 1.0,
            text_color: "#ffffff".into(),
            bg_color: None,
            bg_opacity: 60,
        }
    }
}

fn is_hex(s: &str) -> bool {
    s.len() == 7 && s.starts_with('#') && s[1..].chars().all(|c| c.is_ascii_hexdigit())
}

impl Appearance {
    pub fn normalized(mut self) -> Self {
        let d = Appearance::default();
        if !SIZES.iter().any(|s| (s - self.size).abs() < 1e-6) {
            self.size = d.size;
        }
        if !is_hex(&self.text_color) {
            self.text_color = d.text_color;
        }
        if self.bg_color.as_deref().is_some_and(|c| !is_hex(c)) {
            self.bg_color = None;
        }
        if !(10..=100).contains(&self.bg_opacity) {
            self.bg_opacity = d.bg_opacity;
        }
        self
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct TranslationCfg {
    #[serde(deserialize_with = "lenient")]
    pub mode: Mode,
    #[serde(deserialize_with = "lenient")]
    pub target_lang: TargetLang,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct WindowPos {
    pub x: i32,
    pub y: i32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Config {
    #[serde(deserialize_with = "lenient")]
    pub window: Option<WindowPos>,
    #[serde(deserialize_with = "lenient")]
    pub offsets: HashMap<String, i64>,
    #[serde(deserialize_with = "lenient")]
    pub appearance: Appearance,
    #[serde(deserialize_with = "lenient")]
    pub translation: TranslationCfg,
}

impl Config {
    pub fn normalized(mut self) -> Self {
        self.appearance = self.appearance.normalized();
        self
    }
}

pub fn load(path: &Path) -> Config {
    match std::fs::read_to_string(path) {
        Ok(s) => serde_json::from_str::<Config>(&s).unwrap_or_default().normalized(),
        Err(_) => Config::default(),
    }
}

/// Grava de forma atômica (arquivo temporário + rename).
pub fn save(path: &Path, cfg: &Config) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let tmp = path.with_extension("json.tmp");
    let bytes = serde_json::to_vec_pretty(cfg).map_err(std::io::Error::other)?;
    std::fs::write(&tmp, bytes)?;
    std::fs::rename(tmp, path)
}
```

- [ ] **Step 4: Rodar e ver passar**

Run: `cargo test --manifest-path src-tauri/Cargo.toml config::`
Expected: 6 testes PASS.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/config.rs src-tauri/src/lib.rs
git commit -m "feat: config com load/save tolerante a valores inválidos

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 3: `lyrics::lrc` — parse de LRC e `line_at`

**Files:**
- Create: `src-tauri/src/lyrics/mod.rs`, `src-tauri/src/lyrics/lrc.rs`
- Modify: `src-tauri/src/lib.rs` (`pub mod lyrics;`)

**Interfaces:**
- Produces:
  - `struct Line { pub time_ms: u64, pub text: String }`
  - `struct Lyrics { pub lines: Vec<Line> }` com `fn line_at(&self, position_ms: u64) -> Option<usize>` e `fn texts(&self) -> Vec<String>`
  - `fn parse_lrc(input: &str) -> Lyrics`

- [ ] **Step 1: Escrever os testes**

`src-tauri/src/lyrics/mod.rs`:
```rust
pub mod lrc;
```

`src-tauri/src/lyrics/lrc.rs` (testes no fim do arquivo):
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_timestamp_formats() {
        let l = parse_lrc("[00:01.50]dois dígitos\n[00:02.250]três dígitos\n[00:03]sem fração\n[01:04.5]um dígito");
        let t: Vec<u64> = l.lines.iter().map(|x| x.time_ms).collect();
        assert_eq!(t, vec![1500, 2250, 3000, 64500]);
        assert_eq!(l.lines[0].text, "dois dígitos");
    }

    #[test]
    fn multiple_tags_on_one_line_are_sorted() {
        let l = parse_lrc("[00:10.00][00:02.00]refrão inventado\n[00:05.00]verso");
        let got: Vec<(u64, &str)> = l.lines.iter().map(|x| (x.time_ms, x.text.as_str())).collect();
        assert_eq!(got, vec![(2000, "refrão inventado"), (5000, "verso"), (10000, "refrão inventado")]);
    }

    #[test]
    fn ignores_metadata_and_garbage() {
        let l = parse_lrc("[ar:Banda Fictícia]\n[ti:Canção Teste]\n[offset:+100]\nlinha sem tag\n[xx:yy]lixo\n[00:01.00]ok");
        assert_eq!(l.lines.len(), 1);
        assert_eq!(l.lines[0].text, "ok");
    }

    #[test]
    fn keeps_empty_lines_and_handles_crlf() {
        let l = parse_lrc("[00:01.00]primeira\r\n[00:04.00]\r\n[00:06.00] terceira ");
        assert_eq!(l.lines.len(), 3);
        assert_eq!(l.lines[1].text, "");
        assert_eq!(l.lines[2].text, "terceira");
    }

    #[test]
    fn rejects_invalid_seconds_and_signs() {
        let l = parse_lrc("[00:75.00]segundos demais\n[+1:02.00]sinal\n[00:02.1234]fração longa\n[00:03.00]válida");
        assert_eq!(l.texts(), vec!["válida".to_string()]);
    }

    #[test]
    fn empty_input() {
        assert!(parse_lrc("").lines.is_empty());
    }

    #[test]
    fn line_at_boundaries() {
        let l = parse_lrc("[00:01.00]a\n[00:05.00]b\n[00:09.00]c");
        assert_eq!(l.line_at(0), None);
        assert_eq!(l.line_at(999), None);
        assert_eq!(l.line_at(1000), Some(0));
        assert_eq!(l.line_at(4999), Some(0));
        assert_eq!(l.line_at(5000), Some(1));
        assert_eq!(l.line_at(600_000), Some(2));
        assert_eq!(Lyrics::default().line_at(1000), None);
    }

    #[test]
    fn line_at_with_duplicate_timestamps_picks_last() {
        let l = parse_lrc("[00:02.00]x\n[00:02.00]y\n[00:04.00]z");
        assert_eq!(l.line_at(2500), Some(1));
    }
}
```

- [ ] **Step 2: Rodar e ver falhar**

Adicionar `pub mod lyrics;` em `lib.rs`.
Run: `cargo test --manifest-path src-tauri/Cargo.toml lyrics::lrc`
Expected: FAIL de compilação (`parse_lrc` não existe).

- [ ] **Step 3: Implementar**

Topo de `src-tauri/src/lyrics/lrc.rs`:
```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Line {
    pub time_ms: u64,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Lyrics {
    pub lines: Vec<Line>,
}

impl Lyrics {
    /// Índice da última linha com `time_ms <= position_ms`; `None` antes da primeira.
    pub fn line_at(&self, position_ms: u64) -> Option<usize> {
        let idx = self.lines.partition_point(|l| l.time_ms <= position_ms);
        idx.checked_sub(1)
    }

    pub fn texts(&self) -> Vec<String> {
        self.lines.iter().map(|l| l.text.clone()).collect()
    }
}

fn digits(s: &str) -> bool {
    !s.is_empty() && s.chars().all(|c| c.is_ascii_digit())
}

fn parse_timestamp(tag: &str) -> Option<u64> {
    let (min, rest) = tag.split_once(':')?;
    if !digits(min) {
        return None;
    }
    let (sec, frac) = match rest.split_once('.') {
        Some((s, f)) => (s, Some(f)),
        None => (rest, None),
    };
    if !digits(sec) || sec.len() > 2 {
        return None;
    }
    let s: u64 = sec.parse().ok()?;
    if s >= 60 {
        return None;
    }
    let frac_ms = match frac {
        None => 0,
        Some(f) if digits(f) && f.len() <= 3 => {
            let v: u64 = f.parse().ok()?;
            v * 10u64.pow(3 - f.len() as u32)
        }
        Some(_) => return None,
    };
    let m: u64 = min.parse().ok()?;
    Some(m * 60_000 + s * 1000 + frac_ms)
}

pub fn parse_lrc(input: &str) -> Lyrics {
    let mut lines = Vec::new();
    for raw in input.lines() {
        let mut rest = raw;
        let mut times = Vec::new();
        loop {
            let t = rest.trim_start();
            if !t.starts_with('[') {
                break;
            }
            let Some(end) = t.find(']') else { break };
            match parse_timestamp(&t[1..end]) {
                Some(ms) => {
                    times.push(ms);
                    rest = &t[end + 1..];
                }
                None => break,
            }
        }
        if times.is_empty() {
            continue;
        }
        let text = rest.trim().to_string();
        for ms in times {
            lines.push(Line { time_ms: ms, text: text.clone() });
        }
    }
    lines.sort_by_key(|l| l.time_ms);
    Lyrics { lines }
}
```

- [ ] **Step 4: Rodar e ver passar**

Run: `cargo test --manifest-path src-tauri/Cargo.toml lyrics::lrc`
Expected: 8 testes PASS.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/lyrics src-tauri/src/lib.rs
git commit -m "feat: parser LRC e busca binária da linha atual

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 4: `player` (tipos) + `lyrics::lrclib` + cache em memória

**Files:**
- Create: `src-tauri/src/player/mod.rs`, `src-tauri/src/lyrics/lrclib.rs`
- Modify: `src-tauri/src/lyrics/mod.rs`, `src-tauri/src/lib.rs` (`pub mod player;`)

**Interfaces:**
- Consumes: `lyrics::lrc::{parse_lrc, Lyrics}` (Task 3).
- Produces:
  - `player::NowPlaying { title, artist, album: String, duration_ms, position_ms: u64, is_playing: bool }` com `fn key(&self) -> TrackKey` e `fn display_title(&self) -> String` (`"<título> — <artista>"`)
  - `player::TrackKey { artist, title: String, duration_s: u64 }` (Hash/Eq/Clone) com `fn id(&self) -> String` (`"<artist>|<title>|<duration_s>"`)
  - `player::PlayerError(pub String)` (Display), `trait Player: Send + Sync { fn now_playing(&self) -> Result<Option<NowPlaying>, PlayerError>; }`
  - `lyrics::LRCLIB_URL: &str = "https://lrclib.net"`
  - `lyrics::LyricsError::Network(String)`
  - `#[async_trait] trait LyricsSource: Send + Sync { async fn fetch(&self, track: &NowPlaying) -> Result<Option<Lyrics>, LyricsError>; }`
  - `lyrics::lrclib::LrclibClient::new(base: &str)`, `::with_timeout(base: &str, timeout: Duration)`, `async fn fetch(&self, &NowPlaying) -> Result<Option<Lyrics>, LyricsError>`
  - `lyrics::CachedLyrics::new(client: LrclibClient)` implementa `LyricsSource`

- [ ] **Step 1: Tipos do player (sem testes, só tipos)**

`src-tauri/src/player/mod.rs`:
```rust
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NowPlaying {
    pub title: String,
    pub artist: String,
    pub album: String,
    pub duration_ms: u64,
    pub position_ms: u64,
    pub is_playing: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TrackKey {
    pub artist: String,
    pub title: String,
    pub duration_s: u64,
}

impl TrackKey {
    pub fn id(&self) -> String {
        format!("{}|{}|{}", self.artist, self.title, self.duration_s)
    }
}

impl NowPlaying {
    pub fn key(&self) -> TrackKey {
        TrackKey {
            artist: self.artist.clone(),
            title: self.title.clone(),
            duration_s: (self.duration_ms + 500) / 1000,
        }
    }

    pub fn display_title(&self) -> String {
        format!("{} — {}", self.title, self.artist)
    }
}

#[derive(Debug)]
pub struct PlayerError(pub String);

impl fmt::Display for PlayerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

pub trait Player: Send + Sync {
    fn now_playing(&self) -> Result<Option<NowPlaying>, PlayerError>;
}
```

Adicionar `pub mod player;` em `lib.rs`.

- [ ] **Step 2: Escrever os testes do cliente LRCLIB e do cache**

`src-tauri/src/lyrics/mod.rs`:
```rust
pub mod lrc;
pub mod lrclib;

use crate::player::{NowPlaying, TrackKey};
use async_trait::async_trait;
use lrc::Lyrics;
use std::collections::HashMap;
use std::fmt;
use std::sync::Mutex;

pub use lrclib::LrclibClient;

pub const LRCLIB_URL: &str = "https://lrclib.net";

#[derive(Debug)]
pub enum LyricsError {
    Network(String),
}

impl fmt::Display for LyricsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LyricsError::Network(m) => write!(f, "rede: {m}"),
        }
    }
}

#[async_trait]
pub trait LyricsSource: Send + Sync {
    async fn fetch(&self, track: &NowPlaying) -> Result<Option<Lyrics>, LyricsError>;
}

/// Cacheia sucesso e "sem letra"; erros de rede não são cacheados.
pub struct CachedLyrics {
    client: LrclibClient,
    cache: Mutex<HashMap<TrackKey, Option<Lyrics>>>,
}

impl CachedLyrics {
    pub fn new(client: LrclibClient) -> Self {
        Self { client, cache: Mutex::new(HashMap::new()) }
    }
}

#[async_trait]
impl LyricsSource for CachedLyrics {
    async fn fetch(&self, track: &NowPlaying) -> Result<Option<Lyrics>, LyricsError> {
        let key = track.key();
        if let Some(hit) = self.cache.lock().unwrap().get(&key) {
            return Ok(hit.clone());
        }
        let res = self.client.fetch(track).await?;
        self.cache.lock().unwrap().insert(key, res.clone());
        Ok(res)
    }
}
```

`src-tauri/src/lyrics/lrclib.rs` (testes no fim):
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::lyrics::{CachedLyrics, LyricsSource};
    use serde_json::json;
    use wiremock::matchers::{method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn track() -> NowPlaying {
        NowPlaying {
            title: "Canção Teste".into(),
            artist: "Banda Fictícia".into(),
            album: "Álbum Inventado".into(),
            duration_ms: 180_400,
            position_ms: 0,
            is_playing: true,
        }
    }

    const LRC: &str = "[00:01.00]verso inventado um\n[00:04.00]verso inventado dois";

    #[tokio::test]
    async fn get_returns_synced_lyrics() {
        let s = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/get"))
            .and(query_param("artist_name", "Banda Fictícia"))
            .and(query_param("track_name", "Canção Teste"))
            .and(query_param("album_name", "Álbum Inventado"))
            .and(query_param("duration", "180"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({"duration": 180.0, "syncedLyrics": LRC})))
            .expect(1)
            .mount(&s)
            .await;
        let l = LrclibClient::new(&s.uri()).fetch(&track()).await.unwrap().unwrap();
        assert_eq!(l.texts(), vec!["verso inventado um", "verso inventado dois"]);
    }

    #[tokio::test]
    async fn falls_back_to_search_within_3s() {
        let s = MockServer::start().await;
        Mock::given(path("/api/get")).respond_with(ResponseTemplate::new(404)).mount(&s).await;
        Mock::given(path("/api/search"))
            .and(query_param("track_name", "Canção Teste"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!([
                {"duration": 240.0, "syncedLyrics": "[00:01.00]duração errada"},
                {"duration": 182.0, "syncedLyrics": null},
                {"duration": 178.5, "syncedLyrics": "[00:02.00]escolhida"}
            ])))
            .mount(&s)
            .await;
        let l = LrclibClient::new(&s.uri()).fetch(&track()).await.unwrap().unwrap();
        assert_eq!(l.texts(), vec!["escolhida"]);
    }

    #[tokio::test]
    async fn get_without_synced_goes_to_search_and_may_find_nothing() {
        let s = MockServer::start().await;
        Mock::given(path("/api/get"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({"duration": 180.0, "syncedLyrics": null, "plainLyrics": "texto"})))
            .mount(&s)
            .await;
        Mock::given(path("/api/search")).respond_with(ResponseTemplate::new(200).set_body_json(json!([]))).mount(&s).await;
        assert_eq!(LrclibClient::new(&s.uri()).fetch(&track()).await.unwrap(), None);
    }

    #[tokio::test]
    async fn server_error_is_network_error() {
        let s = MockServer::start().await;
        Mock::given(path("/api/get")).respond_with(ResponseTemplate::new(500)).mount(&s).await;
        assert!(LrclibClient::new(&s.uri()).fetch(&track()).await.is_err());
    }

    #[tokio::test]
    async fn timeout_is_network_error() {
        let s = MockServer::start().await;
        Mock::given(path("/api/get"))
            .respond_with(ResponseTemplate::new(200).set_delay(std::time::Duration::from_millis(500)))
            .mount(&s)
            .await;
        let c = LrclibClient::with_timeout(&s.uri(), std::time::Duration::from_millis(100));
        assert!(matches!(c.fetch(&track()).await, Err(LyricsError::Network(_))));
    }

    #[tokio::test]
    async fn cache_hits_and_does_not_cache_errors() {
        let ok = MockServer::start().await;
        Mock::given(path("/api/get"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({"duration": 180.0, "syncedLyrics": LRC})))
            .expect(1)
            .mount(&ok)
            .await;
        let cached = CachedLyrics::new(LrclibClient::new(&ok.uri()));
        assert!(cached.fetch(&track()).await.unwrap().is_some());
        assert!(cached.fetch(&track()).await.unwrap().is_some());

        let bad = MockServer::start().await;
        Mock::given(path("/api/get")).respond_with(ResponseTemplate::new(503)).expect(2).mount(&bad).await;
        let cached = CachedLyrics::new(LrclibClient::new(&bad.uri()));
        assert!(cached.fetch(&track()).await.is_err());
        assert!(cached.fetch(&track()).await.is_err());
    }
}
```

- [ ] **Step 3: Rodar e ver falhar**

Run: `cargo test --manifest-path src-tauri/Cargo.toml lyrics::lrclib`
Expected: FAIL de compilação (`LrclibClient` não existe).

- [ ] **Step 4: Implementar o cliente**

Topo de `src-tauri/src/lyrics/lrclib.rs`:
```rust
use super::lrc::{parse_lrc, Lyrics};
use super::LyricsError;
use crate::player::NowPlaying;
use reqwest::StatusCode;
use serde::Deserialize;
use std::time::Duration;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Record {
    duration: Option<f64>,
    synced_lyrics: Option<String>,
}

pub struct LrclibClient {
    http: reqwest::Client,
    base: String,
}

fn net(e: reqwest::Error) -> LyricsError {
    LyricsError::Network(e.to_string())
}

fn parsed(s: Option<String>) -> Option<Lyrics> {
    let l = parse_lrc(&s?);
    (!l.lines.is_empty()).then_some(l)
}

impl LrclibClient {
    pub fn new(base: &str) -> Self {
        Self::with_timeout(base, Duration::from_secs(5))
    }

    pub fn with_timeout(base: &str, timeout: Duration) -> Self {
        let http = reqwest::Client::builder()
            .timeout(timeout)
            .user_agent(concat!("lyric-overlay/", env!("CARGO_PKG_VERSION"), " (uso pessoal)"))
            .build()
            .expect("cliente HTTP");
        Self { http, base: base.trim_end_matches('/').to_string() }
    }

    pub async fn fetch(&self, t: &NowPlaying) -> Result<Option<Lyrics>, LyricsError> {
        let dur_s = ((t.duration_ms + 500) / 1000).to_string();
        let resp = self
            .http
            .get(format!("{}/api/get", self.base))
            .query(&[
                ("artist_name", t.artist.as_str()),
                ("track_name", t.title.as_str()),
                ("album_name", t.album.as_str()),
                ("duration", dur_s.as_str()),
            ])
            .send()
            .await
            .map_err(net)?;
        if resp.status().is_success() {
            let rec: Record = resp.json().await.map_err(net)?;
            if let Some(l) = parsed(rec.synced_lyrics) {
                return Ok(Some(l));
            }
        } else if resp.status() != StatusCode::NOT_FOUND {
            return Err(LyricsError::Network(format!("HTTP {}", resp.status())));
        }

        let resp = self
            .http
            .get(format!("{}/api/search", self.base))
            .query(&[("artist_name", t.artist.as_str()), ("track_name", t.title.as_str())])
            .send()
            .await
            .map_err(net)?;
        if !resp.status().is_success() {
            return Err(LyricsError::Network(format!("HTTP {}", resp.status())));
        }
        let recs: Vec<Record> = resp.json().await.map_err(net)?;
        let target = t.duration_ms as f64 / 1000.0;
        Ok(recs
            .into_iter()
            .filter(|r| r.duration.is_some_and(|d| (d - target).abs() <= 3.0))
            .find_map(|r| parsed(r.synced_lyrics)))
    }
}
```

- [ ] **Step 5: Rodar e ver passar**

Run: `cargo test --manifest-path src-tauri/Cargo.toml lyrics::`
Expected: todos os testes de `lyrics::` PASS (8 do lrc + 6 do lrclib).

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/player src-tauri/src/lyrics src-tauri/src/lib.rs
git commit -m "feat: cliente LRCLIB com fallback de busca e cache em memória

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 5: `player::macos` — Spotify via AppleScript

**Files:**
- Create: `src-tauri/src/player/macos.rs`, `src-tauri/Info.plist`
- Modify: `src-tauri/src/player/mod.rs`

**Interfaces:**
- Consumes: `NowPlaying`, `Player`, `PlayerError` (Task 4).
- Produces: `player::macos::MacSpotifyPlayer` (unit struct, implementa `Player`), `player::macos::parse_output(&str) -> Result<Option<NowPlaying>, PlayerError>`, `player::system_player() -> Arc<dyn Player>`.

- [ ] **Step 1: Escrever os testes do parser**

`src-tauri/src/player/macos.rs` (testes no fim):
```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn out(fields: [&str; 6]) -> String {
        format!("{}\n", fields.join("\u{1f}"))
    }

    #[test]
    fn parses_playing_track() {
        let np = parse_output(&out(["Canção Teste", "Banda Fictícia", "Álbum Inventado", "180400", "12.5", "playing"]))
            .unwrap()
            .unwrap();
        assert_eq!(np.title, "Canção Teste");
        assert_eq!(np.artist, "Banda Fictícia");
        assert_eq!(np.album, "Álbum Inventado");
        assert_eq!(np.duration_ms, 180_400);
        assert_eq!(np.position_ms, 12_500);
        assert!(np.is_playing);
    }

    #[test]
    fn parses_paused() {
        let np = parse_output(&out(["a", "b", "c", "1000", "0", "paused"])).unwrap().unwrap();
        assert!(!np.is_playing);
    }

    #[test]
    fn empty_output_is_none() {
        assert_eq!(parse_output("").unwrap(), None);
        assert_eq!(parse_output("\n").unwrap(), None);
    }

    #[test]
    fn decimal_comma_locale() {
        let np = parse_output(&out(["a", "b", "c", "215000", "12,345", "playing"])).unwrap().unwrap();
        assert_eq!(np.position_ms, 12_345);
        let np = parse_output(&out(["a", "b", "c", "2,15E+5", "1,5", "playing"])).unwrap().unwrap();
        assert_eq!(np.duration_ms, 215_000);
        assert_eq!(np.position_ms, 1_500);
    }

    #[test]
    fn title_with_pipe_is_kept() {
        let np = parse_output(&out(["Parte 1 | Parte 2", "b", "c", "1000", "0", "playing"])).unwrap().unwrap();
        assert_eq!(np.title, "Parte 1 | Parte 2");
    }

    #[test]
    fn wrong_field_count_is_error() {
        assert!(parse_output("só um campo").is_err());
        assert!(parse_output(&out(["a", "b", "c", "x", "0", "playing"])).is_err());
    }
}
```

- [ ] **Step 2: Rodar e ver falhar**

Adicionar em `player/mod.rs` (depois dos `use`): `pub mod macos;`
Run: `cargo test --manifest-path src-tauri/Cargo.toml player::macos`
Expected: FAIL de compilação (`parse_output` não existe).

- [ ] **Step 3: Implementar**

Topo de `src-tauri/src/player/macos.rs`:
```rust
use super::{NowPlaying, Player, PlayerError};
use std::process::Command;

const SCRIPT: &str = r#"if application "Spotify" is running then
  tell application "Spotify"
    if player state is stopped then return ""
    set sep to (ASCII character 31)
    set t to current track
    return (name of t) & sep & (artist of t) & sep & (album of t) & sep & ((duration of t) as text) & sep & ((player position) as text) & sep & ((player state) as text)
  end tell
else
  return ""
end if"#;

pub struct MacSpotifyPlayer;

impl Player for MacSpotifyPlayer {
    fn now_playing(&self) -> Result<Option<NowPlaying>, PlayerError> {
        let out = Command::new("osascript")
            .arg("-e")
            .arg(SCRIPT)
            .output()
            .map_err(|e| PlayerError(format!("osascript: {e}")))?;
        if !out.status.success() {
            return Err(PlayerError(String::from_utf8_lossy(&out.stderr).trim().to_string()));
        }
        parse_output(&String::from_utf8_lossy(&out.stdout))
    }
}

/// AppleScript usa o separador decimal do sistema (vírgula no pt-BR).
fn parse_num(s: &str) -> Result<f64, PlayerError> {
    s.trim()
        .replace(',', ".")
        .parse::<f64>()
        .map_err(|_| PlayerError(format!("número inválido do osascript: {s:?}")))
}

pub fn parse_output(s: &str) -> Result<Option<NowPlaying>, PlayerError> {
    let s = s.trim_end_matches(['\n', '\r']);
    if s.trim().is_empty() {
        return Ok(None);
    }
    let parts: Vec<&str> = s.split('\u{1f}').collect();
    if parts.len() != 6 {
        return Err(PlayerError(format!("saída inesperada do osascript: {} campos", parts.len())));
    }
    Ok(Some(NowPlaying {
        title: parts[0].to_string(),
        artist: parts[1].to_string(),
        album: parts[2].to_string(),
        duration_ms: parse_num(parts[3])?.round().max(0.0) as u64,
        position_ms: (parse_num(parts[4])? * 1000.0).round().max(0.0) as u64,
        is_playing: parts[5].trim() == "playing",
    }))
}
```

No fim de `src-tauri/src/player/mod.rs`:
```rust
#[cfg(windows)]
pub mod windows;

#[cfg(target_os = "macos")]
pub fn system_player() -> std::sync::Arc<dyn Player> {
    std::sync::Arc::new(macos::MacSpotifyPlayer)
}

#[cfg(windows)]
pub fn system_player() -> std::sync::Arc<dyn Player> {
    std::sync::Arc::new(windows::WinSmtcPlayer)
}

#[cfg(not(any(target_os = "macos", windows)))]
compile_error!("plataforma não suportada");
```

> Para compilar no macOS antes da Task 6, crie `src-tauri/src/player/windows.rs` vazio agora (ele só é compilado no Windows).

`src-tauri/Info.plist` (o Tauri mescla no bundle; o macOS mostra este texto no pedido de permissão de Automação):
```xml
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>NSAppleEventsUsageDescription</key>
  <string>O Lyric Overlay lê a música atual do Spotify para mostrar a letra sincronizada.</string>
</dict>
</plist>
```

- [ ] **Step 4: Rodar e ver passar**

Run: `cargo test --manifest-path src-tauri/Cargo.toml player::`
Expected: 6 testes PASS.

- [ ] **Step 5: Smoke test manual (macOS)**

Com o Spotify tocando qualquer música:
Run: `osascript -e 'tell application "Spotify" to (name of current track) & " / " & (player position as text)'`
Expected: nome da faixa e posição. Se aparecer erro -1743, liberar o Terminal em Ajustes do Sistema → Privacidade e Segurança → Automação → Spotify.

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/player src-tauri/Info.plist
git commit -m "feat: leitura do Spotify no macOS via AppleScript

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 6: `player::windows` — Spotify via SMTC

**Files:**
- Modify: `src-tauri/src/player/windows.rs`, `src-tauri/src/player/mod.rs`

**Interfaces:**
- Consumes: `NowPlaying`, `Player`, `PlayerError`.
- Produces: `player::windows::WinSmtcPlayer` (só em `cfg(windows)`); `player::smtc_elapsed_ms(universal_time: i64, now_unix_ms: u64) -> u64` (puro, compilado em todas as plataformas).

- [ ] **Step 1: Escrever o teste da correção de tempo (roda no macOS também)**

No fim de `src-tauri/src/player/mod.rs`:
```rust
#[cfg(test)]
mod tests {
    use super::*;

    // 2026-01-01T00:00:00Z em ms Unix e em ticks de 100 ns desde 1601.
    const UNIX_MS: u64 = 1_767_225_600_000;
    const WIN_TICKS: i64 = (1_767_225_600_000 + 11_644_473_600_000) * 10_000;

    #[test]
    fn smtc_elapsed() {
        assert_eq!(smtc_elapsed_ms(WIN_TICKS, UNIX_MS), 0);
        assert_eq!(smtc_elapsed_ms(WIN_TICKS, UNIX_MS + 2_500), 2_500);
        // relógio "voltou": nunca negativo
        assert_eq!(smtc_elapsed_ms(WIN_TICKS, UNIX_MS - 1_000), 0);
    }
}
```

- [ ] **Step 2: Rodar e ver falhar**

Run: `cargo test --manifest-path src-tauri/Cargo.toml player::tests`
Expected: FAIL (`smtc_elapsed_ms` não existe).

- [ ] **Step 3: Implementar a função pura**

Em `src-tauri/src/player/mod.rs`, antes de `system_player`:
```rust
/// Tempo decorrido desde um `DateTime.UniversalTime` do WinRT (ticks de 100 ns desde 1601-01-01 UTC).
pub fn smtc_elapsed_ms(universal_time: i64, now_unix_ms: u64) -> u64 {
    const EPOCH_DIFF_MS: i64 = 11_644_473_600_000;
    let then_unix_ms = universal_time / 10_000 - EPOCH_DIFF_MS;
    (now_unix_ms as i64 - then_unix_ms).max(0) as u64
}
```

Run: `cargo test --manifest-path src-tauri/Cargo.toml player::tests`
Expected: PASS.

- [ ] **Step 4: Implementar o WinSmtcPlayer**

`src-tauri/src/player/windows.rs`:
```rust
use super::{smtc_elapsed_ms, NowPlaying, Player, PlayerError};
use ::windows::Media::Control::{
    GlobalSystemMediaTransportControlsSessionManager as Manager,
    GlobalSystemMediaTransportControlsSessionPlaybackStatus as Status,
};
use std::time::{SystemTime, UNIX_EPOCH};

pub struct WinSmtcPlayer;

fn err(e: ::windows::core::Error) -> PlayerError {
    PlayerError(format!("SMTC: {e}"))
}

impl Player for WinSmtcPlayer {
    fn now_playing(&self) -> Result<Option<NowPlaying>, PlayerError> {
        let mgr = Manager::RequestAsync().map_err(err)?.get().map_err(err)?;
        let sessions = mgr.GetSessions().map_err(err)?;
        for s in sessions {
            let id = s.SourceAppUserModelId().map_err(err)?.to_string();
            if !id.to_lowercase().contains("spotify") {
                continue;
            }
            let props = s.TryGetMediaPropertiesAsync().map_err(err)?.get().map_err(err)?;
            let tl = s.GetTimelineProperties().map_err(err)?;
            let status = s.GetPlaybackInfo().map_err(err)?.PlaybackStatus().map_err(err)?;
            let is_playing = status == Status::Playing;
            let duration_ms = (tl.EndTime().map_err(err)?.Duration / 10_000).max(0) as u64;
            let mut position_ms = (tl.Position().map_err(err)?.Duration / 10_000).max(0) as u64;
            if is_playing {
                let now = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0);
                position_ms += smtc_elapsed_ms(tl.LastUpdatedTime().map_err(err)?.UniversalTime, now);
            }
            if duration_ms > 0 {
                position_ms = position_ms.min(duration_ms);
            }
            return Ok(Some(NowPlaying {
                title: props.Title().map_err(err)?.to_string(),
                artist: props.Artist().map_err(err)?.to_string(),
                album: props.AlbumTitle().map_err(err)?.to_string(),
                duration_ms,
                position_ms,
                is_playing,
            }));
        }
        Ok(None)
    }
}
```

- [ ] **Step 5: Checagem cruzada (opcional no macOS)**

Run:
```bash
rustup target add x86_64-pc-windows-msvc
cargo check --manifest-path src-tauri/Cargo.toml --target x86_64-pc-windows-msvc
```
Expected: sem erros em `player/windows.rs`. Se o check cruzado falhar por dependência nativa do Tauri (não por este arquivo), registre isso e valide no Windows na Task 17.

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/player
git commit -m "feat: leitura do Spotify no Windows via SMTC

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 7: `translate::deepl` — cliente DeepL em lotes

**Files:**
- Create: `src-tauri/src/translate/mod.rs`, `src-tauri/src/translate/deepl.rs`
- Modify: `src-tauri/src/lib.rs` (`pub mod translate;`)

**Interfaces:**
- Produces:
  - `translate::Translated { pub lines: Vec<String>, pub source_lang: String }`
  - `translate::TranslateError { InvalidKey, QuotaExceeded, Network(String), Unexpected(String) }` (Debug, PartialEq)
  - `#[async_trait] trait Translator: Send + Sync { async fn translate(&self, lines: &[String], target: &str) -> Result<Translated, TranslateError>; }`
  - `translate::same_language(source: &str, target: &str) -> bool`
  - `translate::deepl::{DEEPL_FREE_URL, DeepLTranslator, Usage}`; `DeepLTranslator::new(key: String)`, `::with_base(base: &str, key: String, timeout: Duration)`, `async fn usage(&self) -> Result<Usage, TranslateError>`; `Usage { character_count: u64, character_limit: u64 }` (Serialize/Deserialize)

- [ ] **Step 1: Escrever os testes**

`src-tauri/src/translate/mod.rs`:
```rust
pub mod deepl;

use async_trait::async_trait;

#[derive(Debug, Clone, PartialEq)]
pub struct Translated {
    pub lines: Vec<String>,
    pub source_lang: String,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TranslateError {
    InvalidKey,
    QuotaExceeded,
    Network(String),
    Unexpected(String),
}

#[async_trait]
pub trait Translator: Send + Sync {
    async fn translate(&self, lines: &[String], target: &str) -> Result<Translated, TranslateError>;
}

/// "PT" == "PT-BR", "EN" == "EN-US".
pub fn same_language(source: &str, target: &str) -> bool {
    let base = |s: &str| s.split('-').next().unwrap_or("").to_ascii_uppercase();
    !source.is_empty() && base(source) == base(target)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_language_compares_base() {
        assert!(same_language("PT", "PT-BR"));
        assert!(same_language("en", "EN-US"));
        assert!(!same_language("JA", "PT-BR"));
        assert!(!same_language("", "PT-BR"));
    }
}
```

`src-tauri/src/translate/deepl.rs` (testes no fim):
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{json, Value};
    use std::time::Duration;
    use wiremock::matchers::{header, method, path};
    use wiremock::{Mock, MockServer, Request, Respond, ResponseTemplate};

    /// Responde "tr:<texto>" para cada texto recebido, com idioma detectado fixo.
    struct Echo(&'static str);
    impl Respond for Echo {
        fn respond(&self, req: &Request) -> ResponseTemplate {
            let v: Value = serde_json::from_slice(&req.body).unwrap();
            let items: Vec<Value> = v["text"]
                .as_array()
                .unwrap()
                .iter()
                .map(|t| json!({"detected_source_language": self.0, "text": format!("tr:{}", t.as_str().unwrap())}))
                .collect();
            ResponseTemplate::new(200).set_body_json(json!({ "translations": items }))
        }
    }

    fn client(s: &MockServer) -> DeepLTranslator {
        DeepLTranslator::with_base(&s.uri(), "chave-teste".into(), Duration::from_secs(2))
    }

    fn strs(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    #[tokio::test]
    async fn preserves_empty_lines_and_indices() {
        let s = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v2/translate"))
            .and(header("Authorization", "DeepL-Auth-Key chave-teste"))
            .respond_with(Echo("JA"))
            .expect(1)
            .mount(&s)
            .await;
        let t = client(&s).translate(&strs(&["um", "", "  ", "dois"]), "PT-BR").await.unwrap();
        assert_eq!(t.lines, strs(&["tr:um", "", "", "tr:dois"]));
        assert_eq!(t.source_lang, "JA");
        let body: Value = serde_json::from_slice(&s.received_requests().await.unwrap()[0].body).unwrap();
        assert_eq!(body["target_lang"], "PT-BR");
        assert_eq!(body["text"], json!(["um", "dois"]));
    }

    #[tokio::test]
    async fn splits_into_batches_of_50() {
        let s = MockServer::start().await;
        Mock::given(path("/v2/translate")).respond_with(Echo("JA")).expect(3).mount(&s).await;
        let input: Vec<String> = (0..120).map(|i| format!("linha {i}")).collect();
        let t = client(&s).translate(&input, "PT-BR").await.unwrap();
        assert_eq!(t.lines.len(), 120);
        assert_eq!(t.lines[0], "tr:linha 0");
        assert_eq!(t.lines[119], "tr:linha 119");
    }

    #[tokio::test]
    async fn all_empty_makes_no_request() {
        let s = MockServer::start().await;
        Mock::given(path("/v2/translate")).respond_with(Echo("JA")).expect(0).mount(&s).await;
        let t = client(&s).translate(&strs(&["", ""]), "PT-BR").await.unwrap();
        assert_eq!(t.lines, strs(&["", ""]));
    }

    #[tokio::test]
    async fn maps_403_and_456() {
        let s = MockServer::start().await;
        Mock::given(path("/v2/translate")).respond_with(ResponseTemplate::new(403)).mount(&s).await;
        assert_eq!(client(&s).translate(&strs(&["a"]), "PT-BR").await, Err(TranslateError::InvalidKey));

        let s = MockServer::start().await;
        Mock::given(path("/v2/translate")).respond_with(ResponseTemplate::new(456)).mount(&s).await;
        assert_eq!(client(&s).translate(&strs(&["a"]), "PT-BR").await, Err(TranslateError::QuotaExceeded));
    }

    #[tokio::test]
    async fn timeout_is_network_error() {
        let s = MockServer::start().await;
        Mock::given(path("/v2/translate"))
            .respond_with(ResponseTemplate::new(200).set_delay(Duration::from_millis(500)))
            .mount(&s)
            .await;
        let c = DeepLTranslator::with_base(&s.uri(), "k".into(), Duration::from_millis(100));
        assert!(matches!(c.translate(&strs(&["a"]), "PT-BR").await, Err(TranslateError::Network(_))));
    }

    #[tokio::test]
    async fn count_mismatch_is_error() {
        let s = MockServer::start().await;
        Mock::given(path("/v2/translate"))
            .respond_with(ResponseTemplate::new(200).set_body_json(
                json!({"translations": [{"detected_source_language": "JA", "text": "só uma"}]}),
            ))
            .mount(&s)
            .await;
        let r = client(&s).translate(&strs(&["a", "b"]), "PT-BR").await;
        assert!(matches!(r, Err(TranslateError::Unexpected(_))));
    }

    #[tokio::test]
    async fn usage() {
        let s = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/v2/usage"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({"character_count": 1234, "character_limit": 500000})))
            .mount(&s)
            .await;
        assert_eq!(client(&s).usage().await.unwrap(), Usage { character_count: 1234, character_limit: 500_000 });
    }
}
```

- [ ] **Step 2: Rodar e ver falhar**

Adicionar `pub mod translate;` em `lib.rs`.
Run: `cargo test --manifest-path src-tauri/Cargo.toml translate::`
Expected: FAIL de compilação (`DeepLTranslator` não existe).

- [ ] **Step 3: Implementar**

Topo de `src-tauri/src/translate/deepl.rs`:
```rust
use super::{TranslateError, Translated, Translator};
use async_trait::async_trait;
use reqwest::StatusCode;
use serde::{Deserialize, Serialize};
use std::time::Duration;

pub const DEEPL_FREE_URL: &str = "https://api-free.deepl.com";
const BATCH: usize = 50;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Usage {
    pub character_count: u64,
    pub character_limit: u64,
}

#[derive(Serialize)]
struct Req<'a> {
    text: Vec<&'a str>,
    target_lang: &'a str,
}

#[derive(Deserialize)]
struct Resp {
    translations: Vec<Item>,
}

#[derive(Deserialize)]
struct Item {
    detected_source_language: String,
    text: String,
}

pub struct DeepLTranslator {
    http: reqwest::Client,
    base: String,
    key: String,
}

fn net(e: reqwest::Error) -> TranslateError {
    TranslateError::Network(e.to_string())
}

fn check(status: StatusCode) -> Result<(), TranslateError> {
    match status.as_u16() {
        403 => Err(TranslateError::InvalidKey),
        456 => Err(TranslateError::QuotaExceeded),
        _ if !status.is_success() => Err(TranslateError::Unexpected(format!("HTTP {status}"))),
        _ => Ok(()),
    }
}

impl DeepLTranslator {
    pub fn new(key: String) -> Self {
        Self::with_base(DEEPL_FREE_URL, key, Duration::from_secs(10))
    }

    pub fn with_base(base: &str, key: String, timeout: Duration) -> Self {
        let http = reqwest::Client::builder().timeout(timeout).build().expect("cliente HTTP");
        Self { http, base: base.trim_end_matches('/').to_string(), key }
    }

    fn auth(&self) -> String {
        format!("DeepL-Auth-Key {}", self.key)
    }

    pub async fn usage(&self) -> Result<Usage, TranslateError> {
        let resp = self
            .http
            .get(format!("{}/v2/usage", self.base))
            .header("Authorization", self.auth())
            .send()
            .await
            .map_err(net)?;
        check(resp.status())?;
        resp.json().await.map_err(net)
    }
}

#[async_trait]
impl Translator for DeepLTranslator {
    async fn translate(&self, lines: &[String], target: &str) -> Result<Translated, TranslateError> {
        let idx: Vec<usize> = lines
            .iter()
            .enumerate()
            .filter(|(_, l)| !l.trim().is_empty())
            .map(|(i, _)| i)
            .collect();
        let mut out = vec![String::new(); lines.len()];
        let mut source_lang = String::new();
        for chunk in idx.chunks(BATCH) {
            let req = Req { text: chunk.iter().map(|&i| lines[i].as_str()).collect(), target_lang: target };
            let resp = self
                .http
                .post(format!("{}/v2/translate", self.base))
                .header("Authorization", self.auth())
                .json(&req)
                .send()
                .await
                .map_err(net)?;
            check(resp.status())?;
            let body: Resp = resp.json().await.map_err(net)?;
            if body.translations.len() != chunk.len() {
                return Err(TranslateError::Unexpected(format!(
                    "DeepL devolveu {} de {} linhas",
                    body.translations.len(),
                    chunk.len()
                )));
            }
            for (&i, item) in chunk.iter().zip(body.translations) {
                if source_lang.is_empty() {
                    source_lang = item.detected_source_language;
                }
                out[i] = item.text;
            }
        }
        Ok(Translated { lines: out, source_lang })
    }
}
```

- [ ] **Step 4: Rodar e ver passar**

Run: `cargo test --manifest-path src-tauri/Cargo.toml translate::`
Expected: 8 testes PASS.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/translate src-tauri/src/lib.rs
git commit -m "feat: cliente DeepL com lotes de 50 e índices preservados

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 8: `translate::cache` — cache de tradução em disco

**Files:**
- Create: `src-tauri/src/translate/cache.rs`
- Modify: `src-tauri/src/translate/mod.rs` (`pub mod cache;`)

**Interfaces:**
- Consumes: `player::TrackKey` (Task 4).
- Produces: `CachedTranslation { source_lang: String, lines: Option<Vec<String>> }` (`None` = "não precisa traduzir"); `DiskCache::new(dir: PathBuf)`, `get(&self, &TrackKey, target: &str) -> Option<CachedTranslation>`, `put(&self, &TrackKey, target: &str, &CachedTranslation) -> io::Result<()>`; `fnv1a(&[u8]) -> u64`.

- [ ] **Step 1: Escrever os testes**

`src-tauri/src/translate/cache.rs` (testes no fim):
```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn key() -> TrackKey {
        TrackKey { artist: "Banda Fictícia".into(), title: "Canção Teste".into(), duration_s: 180 }
    }

    #[test]
    fn fnv_known_vectors() {
        assert_eq!(fnv1a(b""), 0xcbf29ce484222325);
        assert_eq!(fnv1a(b"a"), 0xaf63dc4c8601ec8c);
    }

    #[test]
    fn miss_then_hit() {
        let d = tempfile::tempdir().unwrap();
        let c = DiskCache::new(d.path().join("translations"));
        assert_eq!(c.get(&key(), "PT-BR"), None);
        let v = CachedTranslation { source_lang: "JA".into(), lines: Some(vec!["tr:um".into(), "".into()]) };
        c.put(&key(), "PT-BR", &v).unwrap();
        assert_eq!(c.get(&key(), "PT-BR"), Some(v));
    }

    #[test]
    fn target_is_part_of_key() {
        let d = tempfile::tempdir().unwrap();
        let c = DiskCache::new(d.path().to_path_buf());
        c.put(&key(), "PT-BR", &CachedTranslation { source_lang: "PT".into(), lines: None }).unwrap();
        assert_eq!(c.get(&key(), "ES"), None);
        assert_eq!(c.get(&key(), "PT-BR").unwrap().lines, None);
    }

    #[test]
    fn corrupt_file_is_miss() {
        let d = tempfile::tempdir().unwrap();
        let c = DiskCache::new(d.path().to_path_buf());
        std::fs::write(c.path(&key(), "PT-BR"), "lixo").unwrap();
        assert_eq!(c.get(&key(), "PT-BR"), None);
    }
}
```

- [ ] **Step 2: Rodar e ver falhar**

Adicionar `pub mod cache;` em `translate/mod.rs`.
Run: `cargo test --manifest-path src-tauri/Cargo.toml translate::cache`
Expected: FAIL de compilação.

- [ ] **Step 3: Implementar**

Topo de `src-tauri/src/translate/cache.rs`:
```rust
use crate::player::TrackKey;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CachedTranslation {
    pub source_lang: String,
    /// `None` = mesma língua do alvo, não precisa traduzir.
    pub lines: Option<Vec<String>>,
}

/// FNV-1a 64 bits: estável entre versões do Rust (ao contrário do DefaultHasher).
pub fn fnv1a(bytes: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for b in bytes {
        h ^= *b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

pub struct DiskCache {
    dir: PathBuf,
}

impl DiskCache {
    pub fn new(dir: PathBuf) -> Self {
        Self { dir }
    }

    pub fn path(&self, key: &TrackKey, target: &str) -> PathBuf {
        let h = fnv1a(format!("{}#{}", key.id(), target).as_bytes());
        self.dir.join(format!("{h:016x}.json"))
    }

    pub fn get(&self, key: &TrackKey, target: &str) -> Option<CachedTranslation> {
        let s = std::fs::read_to_string(self.path(key, target)).ok()?;
        serde_json::from_str(&s).ok()
    }

    pub fn put(&self, key: &TrackKey, target: &str, v: &CachedTranslation) -> std::io::Result<()> {
        std::fs::create_dir_all(&self.dir)?;
        let bytes = serde_json::to_vec(v).map_err(std::io::Error::other)?;
        std::fs::write(self.path(key, target), bytes)
    }
}
```

- [ ] **Step 4: Rodar e ver passar**

Run: `cargo test --manifest-path src-tauri/Cargo.toml translate::cache`
Expected: 4 testes PASS.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/translate
git commit -m "feat: cache de tradução em disco

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 9: `translate::service` — regras de quando e como traduzir

**Files:**
- Create: `src-tauri/src/translate/service.rs`
- Modify: `src-tauri/src/translate/mod.rs` (`pub mod service;`)

**Interfaces:**
- Consumes: `config::{Mode, TargetLang}`, `DeepLTranslator`, `Translator`, `same_language`, `DiskCache`, `CachedTranslation`, `TrackKey`.
- Produces:
  - `DeepLStatus { Ok, InvalidKey, QuotaExceeded }` (Serialize snake_case: `"ok" | "invalid_key" | "quota_exceeded"`)
  - `TranslateSettings { mode: Mode, target: TargetLang, key: Option<String> }` (Clone)
  - `TranslationService::new(cache_dir: PathBuf, settings: TranslateSettings, deepl_base: &str, timeout: Duration, on_status: Box<dyn Fn(DeepLStatus) + Send + Sync>)`
  - métodos: `settings() -> TranslateSettings`, `set_mode_target(mode, target)`, `set_key(Option<String>)` (zera status para `Ok`), `status() -> DeepLStatus`, `async translate_track(&self, &TrackKey, &[String]) -> Option<Vec<String>>`, `async usage(&self) -> Result<Usage, TranslateError>`

- [ ] **Step 1: Escrever os testes**

`src-tauri/src/translate/service.rs` (testes no fim):
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{Mode, TargetLang};
    use serde_json::{json, Value};
    use std::sync::{Arc, Mutex};
    use wiremock::matchers::path;
    use wiremock::{Mock, MockServer, Request, Respond, ResponseTemplate};

    struct Echo(&'static str);
    impl Respond for Echo {
        fn respond(&self, req: &Request) -> ResponseTemplate {
            let v: Value = serde_json::from_slice(&req.body).unwrap();
            let items: Vec<Value> = v["text"]
                .as_array()
                .unwrap()
                .iter()
                .map(|t| json!({"detected_source_language": self.0, "text": format!("tr:{}", t.as_str().unwrap())}))
                .collect();
            ResponseTemplate::new(200).set_body_json(json!({ "translations": items }))
        }
    }

    fn key() -> TrackKey {
        TrackKey { artist: "Banda Fictícia".into(), title: "Canção Teste".into(), duration_s: 180 }
    }

    fn lines() -> Vec<String> {
        vec!["um".into(), "".into(), "dois".into()]
    }

    struct Fixture {
        svc: TranslationService,
        statuses: Arc<Mutex<Vec<DeepLStatus>>>,
        _dir: tempfile::TempDir,
    }

    fn fixture(server: &MockServer, mode: Mode, key: Option<&str>) -> Fixture {
        let dir = tempfile::tempdir().unwrap();
        let statuses = Arc::new(Mutex::new(Vec::new()));
        let rec = statuses.clone();
        let svc = TranslationService::new(
            dir.path().join("translations"),
            TranslateSettings { mode, target: TargetLang::PtBr, key: key.map(String::from) },
            &server.uri(),
            Duration::from_millis(300),
            Box::new(move |s| rec.lock().unwrap().push(s)),
        );
        Fixture { svc, statuses, _dir: dir }
    }

    #[tokio::test]
    async fn original_mode_or_no_key_skips() {
        let s = MockServer::start().await;
        Mock::given(path("/v2/translate")).respond_with(Echo("JA")).expect(0).mount(&s).await;
        assert_eq!(fixture(&s, Mode::Original, Some("k")).svc.translate_track(&key(), &lines()).await, None);
        assert_eq!(fixture(&s, Mode::Both, None).svc.translate_track(&key(), &lines()).await, None);
    }

    #[tokio::test]
    async fn translates_once_then_uses_cache() {
        let s = MockServer::start().await;
        Mock::given(path("/v2/translate")).respond_with(Echo("JA")).expect(1).mount(&s).await;
        let f = fixture(&s, Mode::Both, Some("k"));
        let want = Some(vec!["tr:um".to_string(), "".to_string(), "tr:dois".to_string()]);
        assert_eq!(f.svc.translate_track(&key(), &lines()).await, want);
        assert_eq!(f.svc.translate_track(&key(), &lines()).await, want);
    }

    #[tokio::test]
    async fn same_language_is_cached_as_not_needed() {
        let s = MockServer::start().await;
        Mock::given(path("/v2/translate")).respond_with(Echo("PT")).expect(1).mount(&s).await;
        let f = fixture(&s, Mode::Both, Some("k"));
        assert_eq!(f.svc.translate_track(&key(), &lines()).await, None);
        assert_eq!(f.svc.translate_track(&key(), &lines()).await, None);
    }

    #[tokio::test]
    async fn invalid_key_blocks_until_key_changes() {
        let s = MockServer::start().await;
        Mock::given(path("/v2/translate")).respond_with(ResponseTemplate::new(403)).expect(2).mount(&s).await;
        let f = fixture(&s, Mode::Both, Some("errada"));
        assert_eq!(f.svc.translate_track(&key(), &lines()).await, None);
        assert_eq!(f.svc.status(), DeepLStatus::InvalidKey);
        assert_eq!(f.svc.translate_track(&key(), &lines()).await, None); // sem nova requisição
        f.svc.set_key(Some("outra".into()));
        assert_eq!(f.svc.status(), DeepLStatus::Ok);
        f.svc.translate_track(&key(), &lines()).await; // segunda requisição
        assert_eq!(*f.statuses.lock().unwrap(), vec![DeepLStatus::InvalidKey, DeepLStatus::Ok, DeepLStatus::InvalidKey]);
    }

    #[tokio::test]
    async fn quota_exceeded_sets_status() {
        let s = MockServer::start().await;
        Mock::given(path("/v2/translate")).respond_with(ResponseTemplate::new(456)).mount(&s).await;
        let f = fixture(&s, Mode::Translated, Some("k"));
        assert_eq!(f.svc.translate_track(&key(), &lines()).await, None);
        assert_eq!(f.svc.status(), DeepLStatus::QuotaExceeded);
    }

    #[tokio::test]
    async fn network_errors_are_not_cached() {
        let s = MockServer::start().await;
        Mock::given(path("/v2/translate")).respond_with(ResponseTemplate::new(503)).expect(2).mount(&s).await;
        let f = fixture(&s, Mode::Both, Some("k"));
        assert_eq!(f.svc.translate_track(&key(), &lines()).await, None);
        assert_eq!(f.svc.translate_track(&key(), &lines()).await, None);
        assert_eq!(f.svc.status(), DeepLStatus::Ok);
    }

    #[test]
    fn status_serializes_snake_case() {
        assert_eq!(serde_json::to_value(DeepLStatus::InvalidKey).unwrap(), "invalid_key");
        assert_eq!(serde_json::to_value(DeepLStatus::QuotaExceeded).unwrap(), "quota_exceeded");
    }
}
```

- [ ] **Step 2: Rodar e ver falhar**

Adicionar `pub mod service;` em `translate/mod.rs`.
Run: `cargo test --manifest-path src-tauri/Cargo.toml translate::service`
Expected: FAIL de compilação.

- [ ] **Step 3: Implementar**

Topo de `src-tauri/src/translate/service.rs`:
```rust
use super::cache::{CachedTranslation, DiskCache};
use super::deepl::{DeepLTranslator, Usage};
use super::{same_language, TranslateError, Translator};
use crate::config::{Mode, TargetLang};
use crate::player::TrackKey;
use serde::Serialize;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DeepLStatus {
    Ok,
    InvalidKey,
    QuotaExceeded,
}

#[derive(Debug, Clone)]
pub struct TranslateSettings {
    pub mode: Mode,
    pub target: TargetLang,
    pub key: Option<String>,
}

pub struct TranslationService {
    settings: Mutex<TranslateSettings>,
    status: Mutex<DeepLStatus>,
    cache: DiskCache,
    base: String,
    timeout: Duration,
    on_status: Box<dyn Fn(DeepLStatus) + Send + Sync>,
}

impl TranslationService {
    pub fn new(
        cache_dir: PathBuf,
        settings: TranslateSettings,
        deepl_base: &str,
        timeout: Duration,
        on_status: Box<dyn Fn(DeepLStatus) + Send + Sync>,
    ) -> Self {
        Self {
            settings: Mutex::new(settings),
            status: Mutex::new(DeepLStatus::Ok),
            cache: DiskCache::new(cache_dir),
            base: deepl_base.to_string(),
            timeout,
            on_status,
        }
    }

    pub fn settings(&self) -> TranslateSettings {
        self.settings.lock().unwrap().clone()
    }

    pub fn set_mode_target(&self, mode: Mode, target: TargetLang) {
        let mut s = self.settings.lock().unwrap();
        s.mode = mode;
        s.target = target;
    }

    pub fn set_key(&self, key: Option<String>) {
        self.settings.lock().unwrap().key = key;
        self.set_status(DeepLStatus::Ok);
    }

    pub fn status(&self) -> DeepLStatus {
        *self.status.lock().unwrap()
    }

    fn set_status(&self, s: DeepLStatus) {
        *self.status.lock().unwrap() = s;
        (self.on_status)(s);
    }

    fn client(&self, key: String) -> DeepLTranslator {
        DeepLTranslator::with_base(&self.base, key, self.timeout)
    }

    pub async fn translate_track(&self, key: &TrackKey, lines: &[String]) -> Option<Vec<String>> {
        let s = self.settings();
        if s.mode == Mode::Original || self.status() != DeepLStatus::Ok {
            return None;
        }
        let api_key = s.key?;
        let target = s.target.code();
        if let Some(hit) = self.cache.get(key, target) {
            return hit.lines;
        }
        match self.client(api_key).translate(lines, target).await {
            Ok(t) => {
                let entry = CachedTranslation {
                    lines: (!same_language(&t.source_lang, target)).then_some(t.lines),
                    source_lang: t.source_lang,
                };
                if let Err(e) = self.cache.put(key, target, &entry) {
                    eprintln!("cache de tradução: {e}");
                }
                entry.lines
            }
            Err(TranslateError::InvalidKey) => {
                self.set_status(DeepLStatus::InvalidKey);
                None
            }
            Err(TranslateError::QuotaExceeded) => {
                self.set_status(DeepLStatus::QuotaExceeded);
                None
            }
            Err(e) => {
                eprintln!("tradução falhou: {e:?}");
                None
            }
        }
    }

    pub async fn usage(&self) -> Result<Usage, TranslateError> {
        let key = self.settings().key.ok_or(TranslateError::InvalidKey)?;
        self.client(key).usage().await
    }
}
```

- [ ] **Step 4: Rodar e ver passar**

Run: `cargo test --manifest-path src-tauri/Cargo.toml translate::`
Expected: todos PASS (7 novos).

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/translate
git commit -m "feat: serviço de tradução com cache, status da chave e modo

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 10: `sync::Engine` — máquina de estados pura

**Files:**
- Create: `src-tauri/src/sync/mod.rs`
- Modify: `src-tauri/src/lib.rs` (`pub mod sync;`)

**Interfaces:**
- Consumes: `NowPlaying`, `TrackKey`, `Lyrics` (Tasks 3–4).
- Produces:
  - `const SEEK_THRESHOLD_MS: i64 = 1500`, `const OFFSET_STEP_MS: i64 = 250`
  - `enum Effect { Hide, Show, LineChanged(i64), LyricsLoaded(TrackKey, Vec<String>), FetchLyrics(NowPlaying), TrackChanged(Option<String>) }`
  - `Engine::new(offsets: HashMap<String, i64>)`, `on_poll(&mut self, Option<NowPlaying>, Instant) -> Vec<Effect>`, `on_lyrics(&mut self, &TrackKey, Option<Lyrics>) -> Vec<Effect>`, `on_tick(&mut self, Instant) -> Vec<Effect>`, `estimate(&self, Instant) -> Option<u64>`, `adjust_offset(&mut self, i64) -> Option<i64>`, `reset_offset(&mut self) -> Option<i64>`, `offsets(&self) -> &HashMap<String, i64>`, `current_track(&self) -> Option<&TrackKey>`, `current_lines(&self) -> Option<Vec<String>>`

- [ ] **Step 1: Escrever os testes**

`src-tauri/src/sync/mod.rs` (testes no fim):
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::lyrics::lrc::parse_lrc;
    use std::time::Duration;

    fn np(title: &str, pos: u64, playing: bool) -> NowPlaying {
        NowPlaying {
            title: title.into(),
            artist: "Banda Fictícia".into(),
            album: "Álbum Inventado".into(),
            duration_ms: 180_000,
            position_ms: pos,
            is_playing: playing,
        }
    }

    fn lyrics() -> Lyrics {
        parse_lrc("[00:01.00]um\n[00:05.00]dois\n[00:09.00]\n[00:12.00]três")
    }

    fn ms(n: u64) -> Duration {
        Duration::from_millis(n)
    }

    /// Engine já com a faixa "A" tocando a partir de `pos` e letra carregada.
    fn loaded(pos: u64, t0: Instant) -> Engine {
        let mut e = Engine::new(HashMap::new());
        e.on_poll(Some(np("A", pos, true)), t0);
        e.on_lyrics(&np("A", 0, true).key(), Some(lyrics()));
        e
    }

    #[test]
    fn new_track_requests_lyrics() {
        let mut e = Engine::new(HashMap::new());
        let fx = e.on_poll(Some(np("A", 0, true)), Instant::now());
        assert_eq!(
            fx,
            vec![Effect::TrackChanged(Some("A — Banda Fictícia".into())), Effect::FetchLyrics(np("A", 0, true))]
        );
    }

    #[test]
    fn lyrics_loaded_then_tick_shows_current_line() {
        let t0 = Instant::now();
        let mut e = Engine::new(HashMap::new());
        e.on_poll(Some(np("A", 6000, true)), t0);
        let key = np("A", 0, true).key();
        assert_eq!(
            e.on_lyrics(&key, Some(lyrics())),
            vec![Effect::LyricsLoaded(key.clone(), vec!["um".into(), "dois".into(), "".into(), "três".into()])]
        );
        assert_eq!(e.on_tick(t0), vec![Effect::Show, Effect::LineChanged(1)]);
        assert_eq!(e.on_tick(t0 + ms(100)), vec![]);
    }

    #[test]
    fn estimates_between_polls() {
        let t0 = Instant::now();
        let mut e = loaded(4000, t0);
        assert_eq!(e.on_tick(t0), vec![Effect::Show, Effect::LineChanged(0)]);
        assert_eq!(e.estimate(t0 + ms(1500)), Some(5500));
        assert_eq!(e.on_tick(t0 + ms(1500)), vec![Effect::LineChanged(1)]);
    }

    #[test]
    fn before_first_line_is_minus_one() {
        let t0 = Instant::now();
        let mut e = loaded(200, t0);
        assert_eq!(e.on_tick(t0), vec![Effect::Show, Effect::LineChanged(-1)]);
    }

    #[test]
    fn pause_hides_and_resume_shows() {
        let t0 = Instant::now();
        let mut e = loaded(6000, t0);
        e.on_tick(t0);
        assert_eq!(e.on_poll(Some(np("A", 6000, false)), t0 + ms(1000)), vec![Effect::Hide]);
        assert_eq!(e.on_tick(t0 + ms(1100)), vec![]);
        assert_eq!(e.estimate(t0 + ms(5000)), Some(6000));
        e.on_poll(Some(np("A", 6000, true)), t0 + ms(2000));
        assert_eq!(e.on_tick(t0 + ms(2000)), vec![Effect::Show, Effect::LineChanged(1)]);
    }

    #[test]
    fn seek_resyncs() {
        let t0 = Instant::now();
        let mut e = loaded(2000, t0);
        e.on_tick(t0);
        e.on_poll(Some(np("A", 13_000, true)), t0 + ms(1000));
        assert_eq!(e.estimate(t0 + ms(1000)), Some(13_000));
        assert_eq!(e.on_tick(t0 + ms(1000)), vec![Effect::LineChanged(3)]);
    }

    #[test]
    fn small_drift_keeps_estimate() {
        let t0 = Instant::now();
        let mut e = loaded(2000, t0);
        e.on_poll(Some(np("A", 3800, true)), t0 + ms(1000)); // estimado 3000, desvio 800
        assert_eq!(e.estimate(t0 + ms(1000)), Some(3000));
    }

    #[test]
    fn replay_of_same_track_resyncs_without_refetch() {
        let t0 = Instant::now();
        let mut e = loaded(179_000, t0);
        e.on_tick(t0);
        let fx = e.on_poll(Some(np("A", 500, true)), t0 + ms(1000));
        assert!(!fx.iter().any(|f| matches!(f, Effect::FetchLyrics(_))));
        assert_eq!(e.on_tick(t0 + ms(1000)), vec![Effect::LineChanged(-1)]);
    }

    #[test]
    fn track_change_hides_and_ignores_stale_lyrics() {
        let t0 = Instant::now();
        let mut e = loaded(6000, t0);
        e.on_tick(t0);
        let fx = e.on_poll(Some(np("B", 0, true)), t0 + ms(1000));
        assert_eq!(
            fx,
            vec![
                Effect::Hide,
                Effect::TrackChanged(Some("B — Banda Fictícia".into())),
                Effect::FetchLyrics(np("B", 0, true)),
            ]
        );
        // letra da faixa A chega atrasada: ignorada
        assert_eq!(e.on_lyrics(&np("A", 0, true).key(), Some(lyrics())), vec![]);
        assert_eq!(e.on_tick(t0 + ms(1100)), vec![]);
    }

    #[test]
    fn nothing_playing_clears_track() {
        let t0 = Instant::now();
        let mut e = loaded(6000, t0);
        e.on_tick(t0);
        assert_eq!(e.on_poll(None, t0 + ms(1000)), vec![Effect::TrackChanged(None), Effect::Hide]);
        assert_eq!(e.current_track(), None);
    }

    #[test]
    fn ads_and_podcasts_are_treated_as_nothing() {
        let mut e = Engine::new(HashMap::new());
        let mut ad = np("Anúncio", 0, true);
        ad.duration_ms = 0;
        assert_eq!(e.on_poll(Some(ad), Instant::now()), vec![]);
        let mut no_artist = np("Episódio", 0, true);
        no_artist.artist = "  ".into();
        assert_eq!(e.on_poll(Some(no_artist), Instant::now()), vec![]);
        assert_eq!(e.current_track(), None);
    }

    #[test]
    fn missing_lyrics_keeps_hidden() {
        let t0 = Instant::now();
        let mut e = Engine::new(HashMap::new());
        e.on_poll(Some(np("A", 6000, true)), t0);
        assert_eq!(e.on_lyrics(&np("A", 0, true).key(), None), vec![]);
        assert_eq!(e.on_tick(t0), vec![]);
    }

    #[test]
    fn offset_shifts_line_and_clamps_at_zero() {
        let t0 = Instant::now();
        let mut e = loaded(4800, t0);
        assert_eq!(e.on_tick(t0), vec![Effect::Show, Effect::LineChanged(0)]);
        assert_eq!(e.adjust_offset(OFFSET_STEP_MS), Some(250));
        assert_eq!(e.on_tick(t0), vec![Effect::LineChanged(1)]);
        assert_eq!(e.offsets().get(&np("A", 0, true).key().id()), Some(&250));
        assert_eq!(e.reset_offset(), Some(0));
        assert!(e.offsets().is_empty());

        let mut e = loaded(100, t0);
        e.adjust_offset(-500);
        assert_eq!(e.on_tick(t0), vec![Effect::Show, Effect::LineChanged(-1)]);
    }

    #[test]
    fn offset_without_track_is_none() {
        let mut e = Engine::new(HashMap::new());
        assert_eq!(e.adjust_offset(250), None);
    }
}
```

- [ ] **Step 2: Rodar e ver falhar**

Adicionar `pub mod sync;` em `lib.rs`.
Run: `cargo test --manifest-path src-tauri/Cargo.toml sync::`
Expected: FAIL de compilação (`Engine` não existe).

- [ ] **Step 3: Implementar**

Topo de `src-tauri/src/sync/mod.rs`:
```rust
use crate::lyrics::lrc::Lyrics;
use crate::player::{NowPlaying, TrackKey};
use std::collections::HashMap;
use std::time::Instant;

pub const SEEK_THRESHOLD_MS: i64 = 1500;
pub const OFFSET_STEP_MS: i64 = 250;

#[derive(Debug, Clone, PartialEq)]
pub enum Effect {
    Hide,
    Show,
    LineChanged(i64),
    LyricsLoaded(TrackKey, Vec<String>),
    FetchLyrics(NowPlaying),
    TrackChanged(Option<String>),
}

struct Anchor {
    position_ms: u64,
    at: Instant,
    playing: bool,
}

pub struct Engine {
    track: Option<TrackKey>,
    lyrics: Option<Lyrics>,
    anchor: Option<Anchor>,
    last_index: Option<i64>,
    showing: bool,
    offsets: HashMap<String, i64>,
}

/// Anúncios e podcasts não têm letra: duração 0 ou título/artista vazio.
fn is_music(np: &NowPlaying) -> bool {
    np.duration_ms > 0 && !np.title.trim().is_empty() && !np.artist.trim().is_empty()
}

impl Engine {
    pub fn new(offsets: HashMap<String, i64>) -> Self {
        Self { track: None, lyrics: None, anchor: None, last_index: None, showing: false, offsets }
    }

    pub fn offsets(&self) -> &HashMap<String, i64> {
        &self.offsets
    }

    pub fn current_track(&self) -> Option<&TrackKey> {
        self.track.as_ref()
    }

    pub fn current_lines(&self) -> Option<Vec<String>> {
        self.lyrics.as_ref().map(|l| l.texts())
    }

    fn hide(&mut self, fx: &mut Vec<Effect>) {
        if self.showing {
            self.showing = false;
            fx.push(Effect::Hide);
        }
        self.last_index = None;
    }

    fn anchor_to(&mut self, np: &NowPlaying, now: Instant) {
        self.anchor = Some(Anchor { position_ms: np.position_ms, at: now, playing: np.is_playing });
    }

    pub fn on_poll(&mut self, np: Option<NowPlaying>, now: Instant) -> Vec<Effect> {
        let mut fx = Vec::new();
        let Some(np) = np.filter(is_music) else {
            if self.track.take().is_some() {
                self.lyrics = None;
                self.anchor = None;
                fx.push(Effect::TrackChanged(None));
            }
            self.hide(&mut fx);
            return fx;
        };

        let key = np.key();
        if self.track.as_ref() != Some(&key) {
            self.hide(&mut fx);
            self.track = Some(key);
            self.lyrics = None;
            self.anchor_to(&np, now);
            fx.push(Effect::TrackChanged(Some(np.display_title())));
            fx.push(Effect::FetchLyrics(np));
            return fx;
        }

        let est = self.estimate(now).unwrap_or(np.position_ms);
        let drift = np.position_ms as i64 - est as i64;
        let playing_changed = self.anchor.as_ref().is_none_or(|a| a.playing != np.is_playing);
        if playing_changed || drift.abs() > SEEK_THRESHOLD_MS {
            self.anchor_to(&np, now);
        }
        if !np.is_playing {
            self.hide(&mut fx);
        }
        fx
    }

    pub fn estimate(&self, now: Instant) -> Option<u64> {
        let a = self.anchor.as_ref()?;
        Some(if a.playing {
            a.position_ms + now.saturating_duration_since(a.at).as_millis() as u64
        } else {
            a.position_ms
        })
    }

    pub fn on_lyrics(&mut self, key: &TrackKey, lyrics: Option<Lyrics>) -> Vec<Effect> {
        if self.track.as_ref() != Some(key) {
            return Vec::new();
        }
        match lyrics {
            Some(l) if !l.lines.is_empty() => {
                let texts = l.texts();
                self.lyrics = Some(l);
                self.last_index = None;
                vec![Effect::LyricsLoaded(key.clone(), texts)]
            }
            _ => {
                self.lyrics = None;
                let mut fx = Vec::new();
                self.hide(&mut fx);
                fx
            }
        }
    }

    fn current_index(&self, now: Instant) -> Option<i64> {
        let lyrics = self.lyrics.as_ref()?;
        let key = self.track.as_ref()?;
        if !self.anchor.as_ref()?.playing {
            return None;
        }
        let offset = self.offsets.get(&key.id()).copied().unwrap_or(0);
        let pos = (self.estimate(now)? as i64 + offset).max(0) as u64;
        Some(lyrics.line_at(pos).map_or(-1, |i| i as i64))
    }

    pub fn on_tick(&mut self, now: Instant) -> Vec<Effect> {
        let Some(idx) = self.current_index(now) else { return Vec::new() };
        let mut fx = Vec::new();
        if !self.showing {
            self.showing = true;
            fx.push(Effect::Show);
        }
        if self.last_index != Some(idx) {
            self.last_index = Some(idx);
            fx.push(Effect::LineChanged(idx));
        }
        fx
    }

    pub fn adjust_offset(&mut self, delta: i64) -> Option<i64> {
        let id = self.track.as_ref()?.id();
        let v = self.offsets.entry(id.clone()).or_insert(0);
        *v += delta;
        let now = *v;
        if now == 0 {
            self.offsets.remove(&id);
        }
        Some(now)
    }

    pub fn reset_offset(&mut self) -> Option<i64> {
        let id = self.track.as_ref()?.id();
        self.offsets.remove(&id);
        Some(0)
    }
}
```

- [ ] **Step 4: Rodar e ver passar**

Run: `cargo test --manifest-path src-tauri/Cargo.toml sync::`
Expected: 14 testes PASS.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/sync src-tauri/src/lib.rs
git commit -m "feat: engine de sincronização com estimativa, seek e offset

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 11: `sync::runtime` — loop com I/O e teste com FakePlayer

**Files:**
- Create: `src-tauri/src/sync/runtime.rs`
- Modify: `src-tauri/src/sync/mod.rs` (`pub mod runtime;`), `src-tauri/src/translate/service.rs` (impl `TrackTranslator`)

**Interfaces:**
- Consumes: `Engine`, `Effect`, `Player`, `LyricsSource`, `TranslationService`.
- Produces:
  - `enum OverlayEvent { LyricsLoaded(Vec<String>), TranslationLoaded(Vec<String>), LineChanged(i64), Hide, Show, OffsetChanged(i64) }`
  - `trait Sink: Send + Sync { fn emit(&self, OverlayEvent); fn track_changed(&self, Option<String>); fn offsets_changed(&self, &HashMap<String, i64>); }`
  - `#[async_trait] trait TrackTranslator: Send + Sync { async fn translate_track(&self, &TrackKey, &[String]) -> Option<Vec<String>>; }`
  - `enum SyncCmd { AdjustOffset(i64), ResetOffset, Retranslate }`
  - `struct Timing { poll: Duration, tick: Duration }` (`Default` = 1 s / 100 ms)
  - `struct Deps { player: Arc<dyn Player>, lyrics: Arc<dyn LyricsSource>, translator: Arc<dyn TrackTranslator>, sink: Arc<dyn Sink> }`
  - `async fn run(deps: Deps, engine: Engine, cmds: UnboundedReceiver<SyncCmd>, timing: Timing)` — termina quando o `UnboundedSender` é descartado.
  - `TranslationLoaded(vec![])` significa "sem tradução para esta faixa".

- [ ] **Step 1: Escrever o teste**

`src-tauri/src/sync/runtime.rs` (teste no fim):
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::lyrics::lrc::parse_lrc;
    use crate::lyrics::LyricsError;
    use crate::player::PlayerError;
    use std::sync::Mutex;

    struct FakePlayer(Mutex<Option<NowPlaying>>);
    impl Player for FakePlayer {
        fn now_playing(&self) -> Result<Option<NowPlaying>, PlayerError> {
            Ok(self.0.lock().unwrap().clone())
        }
    }

    struct FakeLyrics;
    #[async_trait]
    impl LyricsSource for FakeLyrics {
        async fn fetch(&self, _: &NowPlaying) -> Result<Option<Lyrics>, LyricsError> {
            Ok(Some(parse_lrc("[00:01.00]um\n[00:05.00]dois")))
        }
    }

    struct FakeTranslator;
    #[async_trait]
    impl TrackTranslator for FakeTranslator {
        async fn translate_track(&self, _: &TrackKey, lines: &[String]) -> Option<Vec<String>> {
            Some(lines.iter().map(|l| format!("tr:{l}")).collect())
        }
    }

    #[derive(Default)]
    struct Recorder {
        events: Mutex<Vec<OverlayEvent>>,
        titles: Mutex<Vec<Option<String>>>,
        offsets: Mutex<Vec<HashMap<String, i64>>>,
    }
    impl Sink for Recorder {
        fn emit(&self, ev: OverlayEvent) {
            self.events.lock().unwrap().push(ev);
        }
        fn track_changed(&self, t: Option<String>) {
            self.titles.lock().unwrap().push(t);
        }
        fn offsets_changed(&self, o: &HashMap<String, i64>) {
            self.offsets.lock().unwrap().push(o.clone());
        }
    }

    #[tokio::test]
    async fn full_cycle_with_fakes() {
        let np = NowPlaying {
            title: "Canção Teste".into(),
            artist: "Banda Fictícia".into(),
            album: "Álbum Inventado".into(),
            duration_ms: 180_000,
            position_ms: 6000,
            is_playing: true,
        };
        let rec = Arc::new(Recorder::default());
        let deps = Deps {
            player: Arc::new(FakePlayer(Mutex::new(Some(np)))),
            lyrics: Arc::new(FakeLyrics),
            translator: Arc::new(FakeTranslator),
            sink: rec.clone(),
        };
        let (tx, rx) = mpsc::unbounded_channel();
        let timing = Timing { poll: Duration::from_millis(20), tick: Duration::from_millis(5) };
        let handle = tokio::spawn(run(deps, Engine::new(HashMap::new()), rx, timing));

        tokio::time::sleep(Duration::from_millis(200)).await;
        {
            let ev = rec.events.lock().unwrap();
            assert!(ev.contains(&OverlayEvent::LyricsLoaded(vec!["um".into(), "dois".into()])));
            assert!(ev.contains(&OverlayEvent::TranslationLoaded(vec!["tr:um".into(), "tr:dois".into()])));
            assert!(ev.contains(&OverlayEvent::Show));
            assert!(ev.contains(&OverlayEvent::LineChanged(1)));
        }
        assert_eq!(rec.titles.lock().unwrap()[0], Some("Canção Teste — Banda Fictícia".into()));

        tx.send(SyncCmd::AdjustOffset(250)).unwrap();
        tokio::time::sleep(Duration::from_millis(50)).await;
        assert!(rec.events.lock().unwrap().contains(&OverlayEvent::OffsetChanged(250)));
        assert_eq!(rec.offsets.lock().unwrap().last().unwrap().get("Banda Fictícia|Canção Teste|180"), Some(&250));

        drop(tx);
        tokio::time::timeout(Duration::from_secs(1), handle).await.unwrap().unwrap();
    }
}
```

- [ ] **Step 2: Rodar e ver falhar**

Adicionar `pub mod runtime;` em `sync/mod.rs`.
Run: `cargo test --manifest-path src-tauri/Cargo.toml sync::runtime`
Expected: FAIL de compilação.

- [ ] **Step 3: Implementar o runtime**

Topo de `src-tauri/src/sync/runtime.rs`:
```rust
use super::{Effect, Engine};
use crate::lyrics::lrc::Lyrics;
use crate::lyrics::LyricsSource;
use crate::player::{NowPlaying, Player, TrackKey};
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};
use tokio::time::MissedTickBehavior;

#[derive(Debug, Clone, PartialEq)]
pub enum OverlayEvent {
    LyricsLoaded(Vec<String>),
    TranslationLoaded(Vec<String>),
    LineChanged(i64),
    Hide,
    Show,
    OffsetChanged(i64),
}

pub trait Sink: Send + Sync {
    fn emit(&self, ev: OverlayEvent);
    fn track_changed(&self, title: Option<String>);
    fn offsets_changed(&self, offsets: &HashMap<String, i64>);
}

#[async_trait]
pub trait TrackTranslator: Send + Sync {
    async fn translate_track(&self, key: &TrackKey, lines: &[String]) -> Option<Vec<String>>;
}

#[derive(Debug)]
pub enum SyncCmd {
    AdjustOffset(i64),
    ResetOffset,
    Retranslate,
}

pub struct Timing {
    pub poll: Duration,
    pub tick: Duration,
}

impl Default for Timing {
    fn default() -> Self {
        Self { poll: Duration::from_secs(1), tick: Duration::from_millis(100) }
    }
}

pub struct Deps {
    pub player: Arc<dyn Player>,
    pub lyrics: Arc<dyn LyricsSource>,
    pub translator: Arc<dyn TrackTranslator>,
    pub sink: Arc<dyn Sink>,
}

enum Internal {
    Lyrics(TrackKey, Option<Lyrics>),
    Translation(TrackKey, Option<Vec<String>>),
}

fn spawn_fetch(deps: &Deps, itx: &UnboundedSender<Internal>, np: NowPlaying) {
    let src = deps.lyrics.clone();
    let itx = itx.clone();
    tokio::spawn(async move {
        let res = src.fetch(&np).await.unwrap_or_else(|e| {
            eprintln!("letra: {e}");
            None
        });
        let _ = itx.send(Internal::Lyrics(np.key(), res));
    });
}

fn spawn_translate(deps: &Deps, itx: &UnboundedSender<Internal>, key: TrackKey, lines: Vec<String>) {
    let tr = deps.translator.clone();
    let itx = itx.clone();
    tokio::spawn(async move {
        let res = tr.translate_track(&key, &lines).await;
        let _ = itx.send(Internal::Translation(key, res));
    });
}

fn apply(deps: &Deps, itx: &UnboundedSender<Internal>, fx: Vec<Effect>) {
    for e in fx {
        match e {
            Effect::Hide => deps.sink.emit(OverlayEvent::Hide),
            Effect::Show => deps.sink.emit(OverlayEvent::Show),
            Effect::LineChanged(i) => deps.sink.emit(OverlayEvent::LineChanged(i)),
            Effect::TrackChanged(t) => deps.sink.track_changed(t),
            Effect::FetchLyrics(np) => spawn_fetch(deps, itx, np),
            Effect::LyricsLoaded(key, lines) => {
                deps.sink.emit(OverlayEvent::LyricsLoaded(lines.clone()));
                spawn_translate(deps, itx, key, lines);
            }
        }
    }
}

pub async fn run(deps: Deps, mut engine: Engine, mut cmds: UnboundedReceiver<SyncCmd>, timing: Timing) {
    let (itx, mut irx) = mpsc::unbounded_channel::<Internal>();
    let mut poll = tokio::time::interval(timing.poll);
    poll.set_missed_tick_behavior(MissedTickBehavior::Skip);
    let mut tick = tokio::time::interval(timing.tick);
    tick.set_missed_tick_behavior(MissedTickBehavior::Skip);

    loop {
        tokio::select! {
            _ = poll.tick() => {
                let player = deps.player.clone();
                let np = match tokio::task::spawn_blocking(move || player.now_playing()).await {
                    Ok(Ok(np)) => np,
                    Ok(Err(e)) => { eprintln!("player: {e}"); None }
                    Err(e) => { eprintln!("player (task): {e}"); None }
                };
                let fx = engine.on_poll(np, Instant::now());
                apply(&deps, &itx, fx);
            }
            _ = tick.tick() => {
                let fx = engine.on_tick(Instant::now());
                apply(&deps, &itx, fx);
            }
            Some(msg) = irx.recv() => match msg {
                Internal::Lyrics(key, l) => {
                    let fx = engine.on_lyrics(&key, l);
                    apply(&deps, &itx, fx);
                }
                Internal::Translation(key, lines) => {
                    if engine.current_track() == Some(&key) {
                        deps.sink.emit(OverlayEvent::TranslationLoaded(lines.unwrap_or_default()));
                    }
                }
            },
            cmd = cmds.recv() => match cmd {
                None => break,
                Some(SyncCmd::AdjustOffset(d)) => {
                    if let Some(o) = engine.adjust_offset(d) {
                        deps.sink.emit(OverlayEvent::OffsetChanged(o));
                        deps.sink.offsets_changed(engine.offsets());
                    }
                }
                Some(SyncCmd::ResetOffset) => {
                    if let Some(o) = engine.reset_offset() {
                        deps.sink.emit(OverlayEvent::OffsetChanged(o));
                        deps.sink.offsets_changed(engine.offsets());
                    }
                }
                Some(SyncCmd::Retranslate) => {
                    if let (Some(key), Some(lines)) = (engine.current_track().cloned(), engine.current_lines()) {
                        spawn_translate(&deps, &itx, key, lines);
                    }
                }
            },
        }
    }
}
```

No fim de `src-tauri/src/translate/service.rs` (antes do `mod tests`):
```rust
#[async_trait::async_trait]
impl crate::sync::runtime::TrackTranslator for TranslationService {
    async fn translate_track(&self, key: &TrackKey, lines: &[String]) -> Option<Vec<String>> {
        TranslationService::translate_track(self, key, lines).await
    }
}
```

- [ ] **Step 4: Rodar e ver passar**

Run: `cargo test --manifest-path src-tauri/Cargo.toml`
Expected: todos os testes PASS (inclui `sync::runtime::tests::full_cycle_with_fakes`).

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/sync src-tauri/src/translate/service.rs
git commit -m "feat: loop de sincronização com player, letra e tradução

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 12: `geometry` + `icon` + `links` — funções puras de apoio

**Files:**
- Create: `src-tauri/src/geometry.rs`, `src-tauri/src/icon.rs`, `src-tauri/src/links.rs`
- Modify: `src-tauri/src/lib.rs` (`pub mod geometry; pub mod icon; pub mod links;`)

**Interfaces:**
- Consumes: `config::Mode`.
- Produces:
  - `geometry::Rect { x: i32, y: i32, w: u32, h: u32 }` (pixels físicos)
  - `geometry::overlay_size(scale: f64, mode: Mode, monitor: Rect, scale_factor: f64) -> (u32, u32)`
  - `geometry::default_position(monitor: Rect, size: (u32, u32), scale_factor: f64) -> (i32, i32)`
  - `geometry::keep_center(pos: (i32, i32), old: (u32, u32), new: (u32, u32)) -> (i32, i32)`
  - `geometry::center_on_any(pos: (i32, i32), size: (u32, u32), monitors: &[Rect]) -> bool`
  - `icon::ICON_PX: u32 = 44`, `icon::note_rgba(size: u32) -> Vec<u8>`
  - `links::Link { Support, DeeplSignup, DeeplKeys }` (Deserialize snake_case), `links::SUPPORT_URL`, `links::url(Link) -> &'static str`, `links::open(&AppHandle, Link) -> Result<(), String>`

- [ ] **Step 1: Escrever os testes**

`src-tauri/src/geometry.rs` (testes no fim):
```rust
#[cfg(test)]
mod tests {
    use super::*;

    const FHD: Rect = Rect { x: 0, y: 0, w: 1920, h: 1080 };

    #[test]
    fn sizes() {
        assert_eq!(overlay_size(1.0, Mode::Original, FHD, 1.0), (900, 130));
        assert_eq!(overlay_size(1.0, Mode::Both, FHD, 1.0), (900, 170));
        assert_eq!(overlay_size(0.8, Mode::Translated, FHD, 1.0), (720, 104));
        assert_eq!(overlay_size(1.0, Mode::Original, Rect { w: 3840, h: 2160, ..FHD }, 2.0), (1800, 260));
    }

    #[test]
    fn width_capped_at_90_percent() {
        let small = Rect { x: 0, y: 0, w: 1280, h: 800 };
        assert_eq!(overlay_size(1.5, Mode::Both, small, 1.0), (1152, 255));
    }

    #[test]
    fn default_position_is_bottom_center() {
        assert_eq!(default_position(FHD, (900, 130), 1.0), (510, 830));
        let second = Rect { x: 1920, y: -200, w: 1920, h: 1080 };
        assert_eq!(default_position(second, (900, 130), 1.0), (2430, 630));
        assert_eq!(default_position(Rect { w: 3840, h: 2160, ..FHD }, (1800, 260), 2.0), (1020, 1660));
    }

    #[test]
    fn keep_center_on_resize() {
        assert_eq!(keep_center((510, 830), (900, 130), (1125, 163)), (398, 814));
    }

    #[test]
    fn detects_offscreen() {
        let mons = [FHD, Rect { x: -1280, y: 0, w: 1280, h: 1024 }];
        assert!(center_on_any((510, 830), (900, 130), &mons));
        assert!(center_on_any((-1000, 100), (900, 130), &mons));
        assert!(!center_on_any((5000, 100), (900, 130), &mons));
        assert!(!center_on_any((510, 1100), (900, 130), &mons));
    }
}
```

`src-tauri/src/icon.rs` (testes no fim):
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn icon_has_shape_and_transparent_corners() {
        let px = note_rgba(ICON_PX);
        assert_eq!(px.len(), (ICON_PX * ICON_PX * 4) as usize);
        let alpha = |x: u32, y: u32| px[((y * ICON_PX + x) * 4 + 3) as usize];
        assert_eq!(alpha(0, 0), 0);
        assert_eq!(alpha(ICON_PX - 1, ICON_PX - 1), 0);
        let opaque = px.chunks(4).filter(|p| p[3] == 255).count();
        assert!(opaque > 100 && opaque < (ICON_PX * ICON_PX / 2) as usize, "opaque = {opaque}");
    }
}
```

`src-tauri/src/links.rs` (testes no fim):
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn urls() {
        assert_eq!(url(Link::Support), "https://buymeacoffee.com/luizfbalves");
        assert_eq!(url(Link::DeeplSignup), "https://www.deepl.com/pro-api");
        assert_eq!(url(Link::DeeplKeys), "https://www.deepl.com/your-account/keys");
        let l: Link = serde_json::from_str("\"deepl_keys\"").unwrap();
        assert_eq!(l, Link::DeeplKeys);
    }
}
```

- [ ] **Step 2: Rodar e ver falhar**

Adicionar os três `pub mod` em `lib.rs`.
Run: `cargo test --manifest-path src-tauri/Cargo.toml geometry:: icon:: links::`
(se o cargo não aceitar vários filtros, rodar um por vez)
Expected: FAIL de compilação.

- [ ] **Step 3: Implementar**

Topo de `src-tauri/src/geometry.rs`:
```rust
use crate::config::Mode;

pub const BASE_W: f64 = 900.0;
pub const BASE_H: f64 = 130.0;
pub const BASE_H_BOTH: f64 = 170.0;
pub const BOTTOM_MARGIN: f64 = 120.0;
pub const MAX_W_FRACTION: f64 = 0.9;

/// Retângulo em pixels físicos.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub w: u32,
    pub h: u32,
}

pub fn overlay_size(scale: f64, mode: Mode, monitor: Rect, scale_factor: f64) -> (u32, u32) {
    let base_h = if mode == Mode::Both { BASE_H_BOTH } else { BASE_H };
    let w = (BASE_W * scale * scale_factor).min(monitor.w as f64 * MAX_W_FRACTION);
    let h = base_h * scale * scale_factor;
    (w.round() as u32, h.round() as u32)
}

pub fn default_position(monitor: Rect, size: (u32, u32), scale_factor: f64) -> (i32, i32) {
    let x = monitor.x + (monitor.w as i32 - size.0 as i32) / 2;
    let y = monitor.y + monitor.h as i32 - size.1 as i32 - (BOTTOM_MARGIN * scale_factor).round() as i32;
    (x, y)
}

pub fn keep_center(pos: (i32, i32), old: (u32, u32), new: (u32, u32)) -> (i32, i32) {
    (pos.0 + (old.0 as i32 - new.0 as i32) / 2, pos.1 + (old.1 as i32 - new.1 as i32) / 2)
}

pub fn center_on_any(pos: (i32, i32), size: (u32, u32), monitors: &[Rect]) -> bool {
    let cx = pos.0 + size.0 as i32 / 2;
    let cy = pos.1 + size.1 as i32 / 2;
    monitors
        .iter()
        .any(|m| cx >= m.x && cx < m.x + m.w as i32 && cy >= m.y && cy < m.y + m.h as i32)
}
```

Topo de `src-tauri/src/icon.rs`:
```rust
/// 22 pt @2x para a barra de menus.
pub const ICON_PX: u32 = 44;

/// Duas colcheias ligadas, em preto com alfa (ícone "template" do macOS).
pub fn note_rgba(size: u32) -> Vec<u8> {
    let s = size as f32;
    let mut px = vec![0u8; (size * size * 4) as usize];
    for y in 0..size {
        for x in 0..size {
            let fx = (x as f32 + 0.5) / s;
            let fy = (y as f32 + 0.5) / s;
            let head = |cx: f32, cy: f32| {
                let dx = (fx - cx) / 0.16;
                let dy = (fy - cy) / 0.12;
                dx * dx + dy * dy <= 1.0
            };
            let beam_top = 0.18 - (fx - 0.40) * (0.08 / 0.50);
            let on = head(0.28, 0.78)
                || head(0.72, 0.70)
                || ((0.40..=0.46).contains(&fx) && (0.18..=0.78).contains(&fy))
                || ((0.84..=0.90).contains(&fx) && (0.10..=0.70).contains(&fy))
                || ((0.40..=0.90).contains(&fx) && fy >= beam_top && fy <= beam_top + 0.12);
            if on {
                px[((y * size + x) * 4 + 3) as usize] = 255;
            }
        }
    }
    px
}
```

Topo de `src-tauri/src/links.rs`:
```rust
use serde::Deserialize;
use tauri::AppHandle;
use tauri_plugin_opener::OpenerExt;

pub const SUPPORT_URL: &str = "https://buymeacoffee.com/luizfbalves";
pub const DEEPL_SIGNUP_URL: &str = "https://www.deepl.com/pro-api";
pub const DEEPL_KEYS_URL: &str = "https://www.deepl.com/your-account/keys";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Link {
    Support,
    DeeplSignup,
    DeeplKeys,
}

pub fn url(link: Link) -> &'static str {
    match link {
        Link::Support => SUPPORT_URL,
        Link::DeeplSignup => DEEPL_SIGNUP_URL,
        Link::DeeplKeys => DEEPL_KEYS_URL,
    }
}

pub fn open(app: &AppHandle, link: Link) -> Result<(), String> {
    app.opener().open_url(url(link), None::<&str>).map_err(|e| e.to_string())
}
```

- [ ] **Step 4: Rodar e ver passar**

Run: `cargo test --manifest-path src-tauri/Cargo.toml`
Expected: todos PASS.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/geometry.rs src-tauri/src/icon.rs src-tauri/src/links.rs src-tauri/src/lib.rs
git commit -m "feat: geometria do overlay, ícone da barra de menus e links

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 13: Integração Tauri — estado, sink, comandos, janela, atalhos

**Files:**
- Create: `src-tauri/src/secrets.rs`, `src-tauri/src/state.rs`, `src-tauri/src/sink.rs`, `src-tauri/src/commands.rs`, `src-tauri/src/overlay.rs`, `src-tauri/src/prefs.rs`, `src-tauri/src/shortcuts.rs`, `src-tauri/src/tray.rs` (stub nesta task)
- Modify: `src-tauri/src/lib.rs`

**Interfaces:**
- Consumes: tudo das Tasks 2–12.
- Produces:
  - `state::Snapshot { lines: Vec<String>, translation: Option<Vec<String>>, index: i64, visible: bool }` + `fn apply(&mut self, &OverlayEvent)`
  - `state::AppState { config: Mutex<Config>, config_path: PathBuf, translation: Arc<TranslationService>, cmds: UnboundedSender<SyncCmd>, snapshot: Mutex<Snapshot>, edit_mode: AtomicBool, tray: Mutex<Option<tray::TrayHandles>> }` com `update_config(&self, impl FnOnce(&mut Config))`
  - Comandos: `get_overlay_init`, `get_settings`, `set_appearance { appearance }`, `set_translation { translation }`, `set_deepl_key { key }`, `get_deepl_usage`, `open_link { link }`
  - `commands::apply_translation(&AppHandle, TranslationCfg)`
  - Eventos emitidos: `lyrics-loaded {lines}`, `translation-loaded {lines}`, `line-changed {index}`, `hide`, `show`, `offset-changed {offset_ms}`, `appearance-changed <Appearance>`, `mode-changed {mode}`, `edit-mode {on}`, `deepl-status <DeepLStatus>`
  - `overlay::{setup, apply_geometry, toggle_edit, toggle_visible}`, `prefs::open`, `shortcuts::{plugin, register}`
  - `tray::{TrayHandles, build, set_track_title, set_mode_checks}` (stub aqui, completo na Task 14)

- [ ] **Step 1: Teste do Snapshot**

`src-tauri/src/state.rs` (testes no fim):
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snapshot_follows_events() {
        let mut s = Snapshot::default();
        assert_eq!(s.index, -1);
        s.apply(&OverlayEvent::LyricsLoaded(vec!["um".into(), "dois".into()]));
        s.apply(&OverlayEvent::TranslationLoaded(vec!["tr:um".into(), "tr:dois".into()]));
        s.apply(&OverlayEvent::Show);
        s.apply(&OverlayEvent::LineChanged(1));
        assert_eq!(s.lines.len(), 2);
        assert_eq!(s.translation.as_ref().unwrap()[1], "tr:dois");
        assert!(s.visible);
        assert_eq!(s.index, 1);

        s.apply(&OverlayEvent::TranslationLoaded(vec![]));
        assert_eq!(s.translation, None);
        s.apply(&OverlayEvent::Hide);
        assert!(!s.visible);
        s.apply(&OverlayEvent::LyricsLoaded(vec!["novo".into()]));
        assert_eq!(s.index, -1);
    }
}
```

- [ ] **Step 2: Rodar e ver falhar**

Adicionar `pub mod state;` em `lib.rs`.
Run: `cargo test --manifest-path src-tauri/Cargo.toml state::`
Expected: FAIL de compilação.

- [ ] **Step 3: Implementar state.rs e secrets.rs**

Topo de `src-tauri/src/state.rs`:
```rust
use crate::config::{self, Config};
use crate::sync::runtime::{OverlayEvent, SyncCmd};
use crate::translate::service::TranslationService;
use crate::tray::TrayHandles;
use serde::Serialize;
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc::UnboundedSender;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Snapshot {
    pub lines: Vec<String>,
    pub translation: Option<Vec<String>>,
    pub index: i64,
    pub visible: bool,
}

impl Default for Snapshot {
    fn default() -> Self {
        Self { lines: Vec::new(), translation: None, index: -1, visible: false }
    }
}

impl Snapshot {
    pub fn apply(&mut self, ev: &OverlayEvent) {
        match ev {
            OverlayEvent::LyricsLoaded(l) => {
                self.lines = l.clone();
                self.translation = None;
                self.index = -1;
            }
            OverlayEvent::TranslationLoaded(l) => {
                self.translation = (!l.is_empty()).then(|| l.clone());
            }
            OverlayEvent::LineChanged(i) => self.index = *i,
            OverlayEvent::Hide => self.visible = false,
            OverlayEvent::Show => self.visible = true,
            OverlayEvent::OffsetChanged(_) => {}
        }
    }
}

pub struct AppState {
    pub config: Mutex<Config>,
    pub config_path: PathBuf,
    pub translation: Arc<TranslationService>,
    pub cmds: UnboundedSender<SyncCmd>,
    pub snapshot: Mutex<Snapshot>,
    pub edit_mode: AtomicBool,
    pub tray: Mutex<Option<TrayHandles>>,
}

impl AppState {
    pub fn new(cfg: Config, config_path: PathBuf, translation: Arc<TranslationService>, cmds: UnboundedSender<SyncCmd>) -> Self {
        Self {
            config: Mutex::new(cfg),
            config_path,
            translation,
            cmds,
            snapshot: Mutex::new(Snapshot::default()),
            edit_mode: AtomicBool::new(false),
            tray: Mutex::new(None),
        }
    }

    pub fn config(&self) -> Config {
        self.config.lock().unwrap().clone()
    }

    pub fn update_config(&self, f: impl FnOnce(&mut Config)) {
        let snapshot = {
            let mut c = self.config.lock().unwrap();
            f(&mut c);
            c.clone()
        };
        if let Err(e) = config::save(&self.config_path, &snapshot) {
            eprintln!("salvar config: {e}");
        }
    }
}
```

`src-tauri/src/secrets.rs`:
```rust
use keyring::Entry;

const SERVICE: &str = "dev.luizfbalves.lyricoverlay";
const USER: &str = "deepl-api-key";

pub fn load_key() -> Option<String> {
    let entry = Entry::new(SERVICE, USER).ok()?;
    match entry.get_password() {
        Ok(k) if !k.trim().is_empty() => Some(k),
        Ok(_) | Err(keyring::Error::NoEntry) => None,
        Err(e) => {
            eprintln!("keyring (leitura): {e}");
            None
        }
    }
}

/// String vazia apaga a chave.
pub fn store_key(key: &str) -> Result<(), String> {
    let entry = Entry::new(SERVICE, USER).map_err(|e| e.to_string())?;
    let key = key.trim();
    if key.is_empty() {
        match entry.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(e) => Err(e.to_string()),
        }
    } else {
        entry.set_password(key).map_err(|e| e.to_string())
    }
}
```

- [ ] **Step 4: Stub do tray (completado na Task 14)**

`src-tauri/src/tray.rs`:
```rust
use crate::config::Mode;
use tauri::AppHandle;

pub struct TrayHandles;

pub fn build(_app: &AppHandle) -> tauri::Result<()> {
    Ok(())
}

pub fn set_track_title(_app: &AppHandle, _title: Option<String>) {}

pub fn set_mode_checks(_app: &AppHandle, _mode: Mode) {}
```

- [ ] **Step 5: sink.rs**

`src-tauri/src/sink.rs`:
```rust
use crate::state::AppState;
use crate::sync::runtime::{OverlayEvent, Sink};
use crate::tray;
use serde_json::json;
use std::collections::HashMap;
use tauri::{AppHandle, Emitter, Manager};

pub struct TauriSink {
    app: AppHandle,
}

impl TauriSink {
    pub fn new(app: AppHandle) -> Self {
        Self { app }
    }
}

impl Sink for TauriSink {
    fn emit(&self, ev: OverlayEvent) {
        self.app.state::<AppState>().snapshot.lock().unwrap().apply(&ev);
        let r = match &ev {
            OverlayEvent::LyricsLoaded(l) => self.app.emit("lyrics-loaded", json!({ "lines": l })),
            OverlayEvent::TranslationLoaded(l) => self.app.emit("translation-loaded", json!({ "lines": l })),
            OverlayEvent::LineChanged(i) => self.app.emit("line-changed", json!({ "index": i })),
            OverlayEvent::Hide => self.app.emit("hide", ()),
            OverlayEvent::Show => self.app.emit("show", ()),
            OverlayEvent::OffsetChanged(o) => self.app.emit("offset-changed", json!({ "offset_ms": o })),
        };
        if let Err(e) = r {
            eprintln!("emit: {e}");
        }
    }

    fn track_changed(&self, title: Option<String>) {
        tray::set_track_title(&self.app, title);
    }

    fn offsets_changed(&self, offsets: &HashMap<String, i64>) {
        self.app.state::<AppState>().update_config(|c| c.offsets = offsets.clone());
    }
}
```

- [ ] **Step 6: overlay.rs e prefs.rs**

`src-tauri/src/overlay.rs`:
```rust
use crate::config::WindowPos;
use crate::geometry::{self, Rect};
use crate::state::AppState;
use serde_json::json;
use std::sync::atomic::Ordering;
use tauri::{AppHandle, Emitter, Manager, Monitor, PhysicalPosition, PhysicalSize, WebviewWindow};

pub const LABEL: &str = "overlay";

fn window(app: &AppHandle) -> Option<WebviewWindow> {
    app.get_webview_window(LABEL)
}

fn rect(m: &Monitor) -> Rect {
    Rect { x: m.position().x, y: m.position().y, w: m.size().width, h: m.size().height }
}

pub fn setup(app: &AppHandle) -> tauri::Result<()> {
    let Some(win) = window(app) else { return Ok(()) };
    win.set_ignore_cursor_events(true)?;
    geometry_inner(app, &win, true)?;
    win.show()?;
    Ok(())
}

pub fn apply_geometry(app: &AppHandle) {
    if let Some(win) = window(app) {
        if let Err(e) = geometry_inner(app, &win, false) {
            eprintln!("geometria do overlay: {e}");
        }
    }
}

fn geometry_inner(app: &AppHandle, win: &WebviewWindow, initial: bool) -> tauri::Result<()> {
    let st = app.state::<AppState>();
    let cfg = st.config();
    let monitors: Vec<Rect> = win.available_monitors()?.iter().map(rect).collect();
    let target = if initial { win.primary_monitor()? } else { win.current_monitor()? };
    let Some(mon) = target.or(win.primary_monitor()?) else { return Ok(()) };
    let sf = mon.scale_factor();
    let size = geometry::overlay_size(cfg.appearance.size, cfg.translation.mode, rect(&mon), sf);

    let pos = if initial {
        match cfg.window {
            Some(p) if geometry::center_on_any((p.x, p.y), size, &monitors) => (p.x, p.y),
            _ => geometry::default_position(rect(&mon), size, sf),
        }
    } else {
        let cur = win.outer_position()?;
        let old = win.outer_size()?;
        geometry::keep_center((cur.x, cur.y), (old.width, old.height), size)
    };

    win.set_size(PhysicalSize::new(size.0, size.1))?;
    win.set_position(PhysicalPosition::new(pos.0, pos.1))?;
    if !initial {
        st.update_config(|c| c.window = Some(WindowPos { x: pos.0, y: pos.1 }));
    }
    Ok(())
}

pub fn toggle_edit(app: &AppHandle) {
    let Some(win) = window(app) else { return };
    let st = app.state::<AppState>();
    let on = !st.edit_mode.load(Ordering::SeqCst);
    st.edit_mode.store(on, Ordering::SeqCst);
    if let Err(e) = win.set_ignore_cursor_events(!on) {
        eprintln!("ignore_cursor_events: {e}");
    }
    if on {
        let _ = win.show();
    } else if let Ok(p) = win.outer_position() {
        st.update_config(|c| c.window = Some(WindowPos { x: p.x, y: p.y }));
    }
    let _ = app.emit("edit-mode", json!({ "on": on }));
}

pub fn toggle_visible(app: &AppHandle) {
    let Some(win) = window(app) else { return };
    let r = if win.is_visible().unwrap_or(true) { win.hide() } else { win.show() };
    if let Err(e) = r {
        eprintln!("mostrar/ocultar: {e}");
    }
}
```

`src-tauri/src/prefs.rs`:
```rust
use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindowBuilder};

pub const LABEL: &str = "prefs";

pub fn open(app: &AppHandle) {
    if let Some(w) = app.get_webview_window(LABEL) {
        let _ = w.show();
        let _ = w.set_focus();
        return;
    }
    let built = WebviewWindowBuilder::new(app, LABEL, WebviewUrl::App("prefs.html".into()))
        .title("Preferências")
        .inner_size(560.0, 440.0)
        .resizable(false)
        .build();
    match built {
        Ok(w) => {
            let _ = w.set_focus();
        }
        Err(e) => eprintln!("abrir Preferências: {e}"),
    }
}
```

- [ ] **Step 7: commands.rs**

`src-tauri/src/commands.rs`:
```rust
use crate::config::{Appearance, Mode, TranslationCfg};
use crate::links::{self, Link};
use crate::state::{AppState, Snapshot};
use crate::sync::runtime::SyncCmd;
use crate::translate::deepl::Usage;
use crate::translate::service::DeepLStatus;
use crate::{overlay, secrets, tray};
use serde::Serialize;
use serde_json::json;
use std::sync::atomic::Ordering;
use tauri::{AppHandle, Emitter, Manager, State};

#[derive(Serialize)]
pub struct OverlayInit {
    appearance: Appearance,
    mode: Mode,
    snapshot: Snapshot,
    edit: bool,
}

#[derive(Serialize)]
pub struct SettingsView {
    appearance: Appearance,
    translation: TranslationCfg,
    has_key: bool,
    deepl_status: DeepLStatus,
}

#[tauri::command]
pub fn get_overlay_init(state: State<'_, AppState>) -> OverlayInit {
    let cfg = state.config();
    OverlayInit {
        appearance: cfg.appearance,
        mode: cfg.translation.mode,
        snapshot: state.snapshot.lock().unwrap().clone(),
        edit: state.edit_mode.load(Ordering::SeqCst),
    }
}

#[tauri::command]
pub fn get_settings(state: State<'_, AppState>) -> SettingsView {
    let cfg = state.config();
    SettingsView {
        appearance: cfg.appearance,
        translation: cfg.translation,
        has_key: state.translation.settings().key.is_some(),
        deepl_status: state.translation.status(),
    }
}

#[tauri::command]
pub fn set_appearance(app: AppHandle, state: State<'_, AppState>, appearance: Appearance) {
    let a = appearance.normalized();
    state.update_config(|c| c.appearance = a.clone());
    overlay::apply_geometry(&app);
    let _ = app.emit("appearance-changed", &a);
}

pub fn apply_translation(app: &AppHandle, t: TranslationCfg) {
    let st = app.state::<AppState>();
    st.update_config(|c| c.translation = t);
    st.translation.set_mode_target(t.mode, t.target_lang);
    overlay::apply_geometry(app);
    tray::set_mode_checks(app, t.mode);
    let _ = app.emit("mode-changed", json!({ "mode": t.mode, "target_lang": t.target_lang }));
    let _ = st.cmds.send(SyncCmd::Retranslate);
}

#[tauri::command]
pub fn set_translation(app: AppHandle, translation: TranslationCfg) {
    apply_translation(&app, translation);
}

#[tauri::command]
pub fn set_deepl_key(state: State<'_, AppState>, key: String) -> Result<(), String> {
    secrets::store_key(&key)?;
    let k = key.trim();
    state.translation.set_key((!k.is_empty()).then(|| k.to_string()));
    let _ = state.cmds.send(SyncCmd::Retranslate);
    Ok(())
}

#[tauri::command]
pub async fn get_deepl_usage(state: State<'_, AppState>) -> Result<Usage, String> {
    state.translation.usage().await.map_err(|e| format!("{e:?}"))
}

#[tauri::command]
pub fn open_link(app: AppHandle, link: Link) -> Result<(), String> {
    links::open(&app, link)
}
```

- [ ] **Step 8: shortcuts.rs**

`src-tauri/src/shortcuts.rs`:
```rust
use crate::overlay;
use crate::state::AppState;
use crate::sync::runtime::SyncCmd;
use crate::sync::OFFSET_STEP_MS;
use tauri::plugin::TauriPlugin;
use tauri::{AppHandle, Manager, Wry};
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};

fn primary() -> Modifiers {
    if cfg!(target_os = "macos") {
        Modifiers::SUPER
    } else {
        Modifiers::CONTROL
    }
}

fn edit() -> Shortcut {
    Shortcut::new(Some(primary() | Modifiers::SHIFT), Code::KeyL)
}

fn earlier() -> Shortcut {
    Shortcut::new(Some(primary() | Modifiers::SHIFT), Code::ArrowLeft)
}

fn later() -> Shortcut {
    Shortcut::new(Some(primary() | Modifiers::SHIFT), Code::ArrowRight)
}

fn send(app: &AppHandle, cmd: SyncCmd) {
    let _ = app.state::<AppState>().cmds.send(cmd);
}

pub fn plugin() -> TauriPlugin<Wry> {
    tauri_plugin_global_shortcut::Builder::new()
        .with_handler(|app, sc, ev| {
            if ev.state() != ShortcutState::Pressed {
                return;
            }
            if *sc == edit() {
                overlay::toggle_edit(app);
            } else if *sc == earlier() {
                send(app, SyncCmd::AdjustOffset(-OFFSET_STEP_MS));
            } else if *sc == later() {
                send(app, SyncCmd::AdjustOffset(OFFSET_STEP_MS));
            }
        })
        .build()
}

pub fn register(app: &AppHandle) {
    let gs = app.global_shortcut();
    for sc in [edit(), earlier(), later()] {
        if let Err(e) = gs.register(sc) {
            eprintln!("atalho {sc:?} indisponível: {e}");
        }
    }
}
```

- [ ] **Step 9: lib.rs completo**

`src-tauri/src/lib.rs`:
```rust
pub mod commands;
pub mod config;
pub mod geometry;
pub mod icon;
pub mod links;
pub mod lyrics;
pub mod overlay;
pub mod player;
pub mod prefs;
pub mod secrets;
pub mod shortcuts;
pub mod sink;
pub mod state;
pub mod sync;
pub mod translate;
pub mod tray;

use std::sync::Arc;
use std::time::Duration;
use tauri::{Emitter, Manager};

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(shortcuts::plugin())
        .invoke_handler(tauri::generate_handler![
            commands::get_overlay_init,
            commands::get_settings,
            commands::set_appearance,
            commands::set_translation,
            commands::set_deepl_key,
            commands::get_deepl_usage,
            commands::open_link,
        ])
        .setup(|app| {
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);

            let handle = app.handle().clone();
            let config_path = app.path().app_config_dir()?.join("config.json");
            let cache_dir = app.path().app_cache_dir()?.join("translations");
            let cfg = config::load(&config_path);

            let status_handle = handle.clone();
            let translation = Arc::new(translate::service::TranslationService::new(
                cache_dir,
                translate::service::TranslateSettings {
                    mode: cfg.translation.mode,
                    target: cfg.translation.target_lang,
                    key: secrets::load_key(),
                },
                translate::deepl::DEEPL_FREE_URL,
                Duration::from_secs(10),
                Box::new(move |s| {
                    let _ = status_handle.emit("deepl-status", s);
                }),
            ));

            let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
            let engine = sync::Engine::new(cfg.offsets.clone());
            app.manage(state::AppState::new(cfg, config_path, translation.clone(), tx));

            tray::build(&handle)?;
            overlay::setup(&handle)?;
            shortcuts::register(&handle);

            let deps = sync::runtime::Deps {
                player: player::system_player(),
                lyrics: Arc::new(lyrics::CachedLyrics::new(lyrics::LrclibClient::new(lyrics::LRCLIB_URL))),
                translator: translation,
                sink: Arc::new(sink::TauriSink::new(handle.clone())),
            };
            tauri::async_runtime::spawn(sync::runtime::run(deps, engine, rx, Default::default()));
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("erro ao iniciar o Lyric Overlay");
}
```

- [ ] **Step 10: Verificar**

Run: `cargo test --manifest-path src-tauri/Cargo.toml && cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings`
Expected: todos os testes PASS; clippy sem avisos (corrigir o que aparecer — normalmente imports não usados).

- [ ] **Step 11: Commit**

```bash
git add src-tauri/src
git commit -m "feat: integração Tauri — estado, comandos, janela e atalhos

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 14: `tray` — ícone na barra de menus/bandeja e menu

**Files:**
- Create: `src-tauri/icons/bmc-menu.png`
- Modify: `src-tauri/src/tray.rs` (substitui o stub)

**Interfaces:**
- Consumes: `AppState`, `overlay::{toggle_visible, toggle_edit}`, `prefs::open`, `links::{open, Link}`, `commands::apply_translation`, `icon::{note_rgba, ICON_PX}`, `SyncCmd::ResetOffset`.
- Produces: `TrayHandles { title: MenuItem<Wry>, modes: Vec<(Mode, CheckMenuItem<Wry>)> }`, `build(&AppHandle) -> tauri::Result<()>`, `set_track_title(&AppHandle, Option<String>)`, `set_mode_checks(&AppHandle, Mode)`.

- [ ] **Step 1: Gerar o ícone do BMC para o menu (32 px)**

```bash
sips -Z 32 src/assets/bmc/bmc-logo.png --out src-tauri/icons/bmc-menu.png
sips -g pixelWidth -g pixelHeight src-tauri/icons/bmc-menu.png
```
Expected: maior lado = 32 px.

- [ ] **Step 2: Implementar tray.rs**

`src-tauri/src/tray.rs`:
```rust
use crate::config::{Mode, TranslationCfg};
use crate::icon::{note_rgba, ICON_PX};
use crate::links::{self, Link};
use crate::state::AppState;
use crate::sync::runtime::SyncCmd;
use crate::{commands, overlay, prefs};
use tauri::image::Image;
use tauri::menu::{CheckMenuItem, IconMenuItem, IsMenuItem, Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{AppHandle, Manager, Wry};

const BMC_PNG: &[u8] = include_bytes!("../icons/bmc-menu.png");
const NOTHING_PLAYING: &str = "Nada tocando";

pub struct TrayHandles {
    pub title: MenuItem<Wry>,
    pub modes: Vec<(Mode, CheckMenuItem<Wry>)>,
}

/// O menu é declarado aqui como lista: para adicionar opções futuras, inclua uma entrada
/// e trate o `id` em `handle`.
enum Entry {
    TrackTitle,
    Separator,
    Header(&'static str),
    Action { id: &'static str, label: &'static str, accel: Option<&'static str> },
    ModeCheck(Mode, &'static str),
    Icon { id: &'static str, label: &'static str, png: &'static [u8] },
}

fn entries() -> Vec<Entry> {
    use Entry::*;
    vec![
        TrackTitle,
        Separator,
        Action { id: "toggle-visible", label: "Mostrar/ocultar letra", accel: None },
        Action { id: "edit", label: "Editar posição", accel: None },
        Action { id: "reset-offset", label: "Resetar offset desta faixa", accel: None },
        Separator,
        Header("Tradução"),
        ModeCheck(Mode::Original, "Só original"),
        ModeCheck(Mode::Translated, "Só tradução"),
        ModeCheck(Mode::Both, "Original + tradução"),
        Separator,
        Action { id: "prefs", label: "Preferências…", accel: Some("CmdOrCtrl+,") },
        Separator,
        Icon { id: "support", label: "Buy me a coffee", png: BMC_PNG },
        Separator,
        Action { id: "quit", label: "Sair", accel: None },
    ]
}

fn mode_id(m: Mode) -> &'static str {
    match m {
        Mode::Original => "mode-original",
        Mode::Translated => "mode-translated",
        Mode::Both => "mode-both",
    }
}

fn mode_from_id(id: &str) -> Option<Mode> {
    [Mode::Original, Mode::Translated, Mode::Both].into_iter().find(|m| mode_id(*m) == id)
}

pub fn build(app: &AppHandle) -> tauri::Result<()> {
    let st = app.state::<AppState>();
    let current = st.config().translation.mode;
    let mut items: Vec<Box<dyn IsMenuItem<Wry>>> = Vec::new();
    let mut title = None;
    let mut modes = Vec::new();

    for e in entries() {
        match e {
            Entry::TrackTitle => {
                let it = MenuItem::with_id(app, "track", NOTHING_PLAYING, false, None::<&str>)?;
                title = Some(it.clone());
                items.push(Box::new(it));
            }
            Entry::Separator => items.push(Box::new(PredefinedMenuItem::separator(app)?)),
            Entry::Header(label) => items.push(Box::new(MenuItem::new(app, label, false, None::<&str>)?)),
            Entry::Action { id, label, accel } => items.push(Box::new(MenuItem::with_id(app, id, label, true, accel)?)),
            Entry::ModeCheck(m, label) => {
                let it = CheckMenuItem::with_id(app, mode_id(m), label, true, m == current, None::<&str>)?;
                modes.push((m, it.clone()));
                items.push(Box::new(it));
            }
            Entry::Icon { id, label, png } => {
                let icon = Image::from_bytes(png)?;
                items.push(Box::new(IconMenuItem::with_id(app, id, label, true, Some(icon), None::<&str>)?));
            }
        }
    }

    let refs: Vec<&dyn IsMenuItem<Wry>> = items.iter().map(|b| b.as_ref()).collect();
    let menu = Menu::with_items(app, &refs)?;

    TrayIconBuilder::with_id("main")
        .icon(Image::new_owned(note_rgba(ICON_PX), ICON_PX, ICON_PX))
        .icon_as_template(true)
        .tooltip("Lyric Overlay")
        .menu(&menu)
        .show_menu_on_left_click(true)
        .on_menu_event(|app, ev| handle(app, ev.id().as_ref()))
        .build(app)?;

    *st.tray.lock().unwrap() = Some(TrayHandles { title: title.expect("item de título"), modes });
    Ok(())
}

fn handle(app: &AppHandle, id: &str) {
    match id {
        "toggle-visible" => overlay::toggle_visible(app),
        "edit" => overlay::toggle_edit(app),
        "reset-offset" => {
            let _ = app.state::<AppState>().cmds.send(SyncCmd::ResetOffset);
        }
        "prefs" => prefs::open(app),
        "support" => {
            if let Err(e) = links::open(app, Link::Support) {
                eprintln!("abrir link: {e}");
            }
        }
        "quit" => app.exit(0),
        other => {
            if let Some(mode) = mode_from_id(other) {
                let cur = app.state::<AppState>().config().translation;
                commands::apply_translation(app, TranslationCfg { mode, ..cur });
            }
        }
    }
}

pub fn set_track_title(app: &AppHandle, title: Option<String>) {
    let st = app.state::<AppState>();
    if let Some(h) = st.tray.lock().unwrap().as_ref() {
        let _ = h.title.set_text(title.as_deref().unwrap_or(NOTHING_PLAYING));
    }
}

pub fn set_mode_checks(app: &AppHandle, mode: Mode) {
    let st = app.state::<AppState>();
    if let Some(h) = st.tray.lock().unwrap().as_ref() {
        for (m, it) in &h.modes {
            let _ = it.set_checked(*m == mode);
        }
    }
}
```

- [ ] **Step 3: Verificar compilação**

Run: `cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings && cargo test --manifest-path src-tauri/Cargo.toml`
Expected: sem erros/avisos; testes PASS.

- [ ] **Step 4: Verificação manual do menu (macOS)**

Run: `npm run tauri dev`
Expected:
- ícone de colcheias na barra de menus, adaptando a tema claro/escuro; app fora do Dock;
- menu na ordem: "Nada tocando" (cinza) · Mostrar/ocultar letra · Editar posição · Resetar offset desta faixa · Tradução (cinza) · 3 itens com ✓ em "Original + tradução" · Preferências… (⌘,) · Buy me a coffee (com logo) · Sair;
- "Buy me a coffee" abre `https://buymeacoffee.com/luizfbalves` no navegador; "Sair" encerra.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/tray.rs src-tauri/icons/bmc-menu.png
git commit -m "feat: ícone na barra de menus/bandeja com menu de opções

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 15: Frontend do overlay

**Files:**
- Create: `src/shared/types.ts`, `src/shared/fonts.ts`, `src/overlay/style.css`
- Modify: `src/index.html`, `src/overlay/main.ts`

**Interfaces:**
- Consumes: comando `get_overlay_init`; eventos `lyrics-loaded`, `translation-loaded`, `line-changed`, `hide`, `show`, `appearance-changed`, `mode-changed`, `offset-changed`, `edit-mode`.
- Produces: `src/shared/types.ts` (tipos espelhando o Rust) e `src/shared/fonts.ts` (`FONT_STACKS`, `FONT_LABELS`, `hexToRgba`) usados também pela Task 16.

- [ ] **Step 1: Tipos compartilhados**

`src/shared/types.ts`:
```ts
export type FontId = "system" | "rounded" | "serif" | "mono" | "handwritten";
export type Mode = "original" | "translated" | "both";
export type TargetLang = "PT-BR" | "EN-US" | "ES";
export type DeepLStatus = "ok" | "invalid_key" | "quota_exceeded";

export interface Appearance {
  font: FontId;
  size: number;
  text_color: string;
  bg_color: string | null;
  bg_opacity: number;
}

export interface TranslationCfg {
  mode: Mode;
  target_lang: TargetLang;
}

export interface Snapshot {
  lines: string[];
  translation: string[] | null;
  index: number;
  visible: boolean;
}

export interface OverlayInit {
  appearance: Appearance;
  mode: Mode;
  snapshot: Snapshot;
  edit: boolean;
}

export interface SettingsView {
  appearance: Appearance;
  translation: TranslationCfg;
  has_key: boolean;
  deepl_status: DeepLStatus;
}

export interface Usage {
  character_count: number;
  character_limit: number;
}

export const DEFAULT_APPEARANCE: Appearance = {
  font: "system",
  size: 1,
  text_color: "#ffffff",
  bg_color: null,
  bg_opacity: 60,
};
```

- [ ] **Step 2: Fontes**

`src/shared/fonts.ts`:
```ts
import "@fontsource/nunito/700.css";
import "@fontsource/lora/600.css";
import "@fontsource/jetbrains-mono/600.css";
import "@fontsource/caveat/600.css";
import "@fontsource/yomogi/400.css";
import type { FontId } from "./types";

export const FONT_STACKS: Record<FontId, string> = {
  system: '-apple-system,"SF Pro Display","Segoe UI",system-ui,sans-serif',
  rounded: '"Nunito",ui-rounded,system-ui,sans-serif',
  serif: '"Lora",Georgia,serif',
  mono: '"JetBrains Mono",ui-monospace,Consolas,monospace',
  handwritten: '"Caveat","Yomogi",cursive',
};

export const FONT_LABELS: Record<FontId, string> = {
  system: "Sistema",
  rounded: "Arredondada",
  serif: "Serifada",
  mono: "Mono",
  handwritten: "Manuscrita",
};

export function hexToRgba(hex: string, opacity: number): string {
  const n = parseInt(hex.slice(1), 16);
  return `rgba(${(n >> 16) & 255},${(n >> 8) & 255},${n & 255},${opacity / 100})`;
}
```

- [ ] **Step 3: HTML e CSS do overlay**

`src/index.html`:
```html
<!doctype html>
<html lang="pt-BR">
  <head>
    <meta charset="UTF-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1" />
    <title>Lyric Overlay</title>
  </head>
  <body>
    <div id="overlay" class="hidden m-orig">
      <div class="viewport" id="viewport"><div class="track" id="track"></div></div>
      <div class="toast" id="toast"></div>
    </div>
    <script type="module" src="./overlay/main.ts"></script>
  </body>
</html>
```

`src/overlay/style.css`:
```css
html, body {
  margin: 0;
  height: 100%;
  background: transparent;
  overflow: hidden;
  user-select: none;
  -webkit-user-select: none;
  cursor: default;
}

#overlay {
  --ov-scale: 1;
  --ov-font: -apple-system, "Segoe UI", system-ui, sans-serif;
  --ov-color: #fff;
  --ov-bg: transparent;
  position: fixed;
  inset: 0;
  box-sizing: border-box;
  padding: 0 16px;
  border: 2px dashed transparent;
  border-radius: 12px;
}
#overlay.edit { border-color: rgba(255, 255, 255, 0.7); background: rgba(0, 0, 0, 0.25); cursor: grab; }

.viewport {
  position: relative;
  height: 100%;
  overflow: hidden;
  transition: opacity 0.3s ease;
  -webkit-mask-image: linear-gradient(transparent 0%, #000 32%, #000 68%, transparent 100%);
  mask-image: linear-gradient(transparent 0%, #000 32%, #000 68%, transparent 100%);
}
#overlay.hidden .viewport { opacity: 0; }

.track { position: relative; will-change: transform; transition: transform 0.55s cubic-bezier(0.22, 0.8, 0.24, 1); }

.ln {
  display: flex;
  flex-direction: column;
  align-items: center;
  width: fit-content;
  max-width: 100%;
  margin: 0 auto;
  box-sizing: border-box;
  padding: 0.18em 0.7em;
  border-radius: 10px;
  font-family: var(--ov-font);
  font-weight: 600;
  font-size: calc(25px * var(--ov-scale));
  line-height: 1.25;
  text-align: center;
  text-wrap: balance;
  color: var(--ov-color);
  text-shadow: 0 1px 2px rgba(0, 0, 0, 0.9), 0 0 12px rgba(0, 0, 0, 0.75);
  opacity: 0.3;
  transform: scale(0.85);
  transition: opacity 0.55s ease, transform 0.55s cubic-bezier(0.22, 0.8, 0.24, 1), background-color 0.4s ease;
}
.ln.cur { opacity: 1; transform: scale(1); }
#overlay.has-bg .ln.cur { background: var(--ov-bg); text-shadow: none; }
.ln.gap { letter-spacing: 0.4em; font-size: calc(17px * var(--ov-scale)); }

.ln .t, .ln .o { display: block; }
.m-orig .ln .t { display: none; }
.m-tr .ln .o { display: none; }
.m-both .ln .o { font-size: 0.62em; font-weight: 500; opacity: 0.8; margin-top: 0.15em; }
.m-both .ln.no-tr .o { display: none; }

.toast {
  position: absolute;
  top: 4px;
  left: 50%;
  transform: translateX(-50%);
  font: 500 12px ui-monospace, Menlo, Consolas, monospace;
  color: #fff;
  background: rgba(0, 0, 0, 0.7);
  padding: 2px 8px;
  border-radius: 6px;
  opacity: 0;
  transition: opacity 0.2s;
  pointer-events: none;
}
.toast.show { opacity: 1; }

@media (prefers-reduced-motion: reduce) {
  .track, .ln, .viewport, .toast { transition: none; }
}
```

- [ ] **Step 4: Lógica do overlay**

`src/overlay/main.ts`:
```ts
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { FONT_STACKS, hexToRgba } from "../shared/fonts";
import type { Appearance, Mode, OverlayInit } from "../shared/types";
import "./style.css";

const root = document.getElementById("overlay")!;
const viewport = document.getElementById("viewport")!;
const track = document.getElementById("track")!;
const toast = document.getElementById("toast")!;

// Texto de exemplo do modo de edição (inventado).
const SAMPLE = ["Letra de exemplo", "Arraste para posicionar", "Cmd/Ctrl+Shift+L para concluir"];
const MODE_CLASS: Record<Mode, string> = { original: "m-orig", translated: "m-tr", both: "m-both" };

let lines: string[] = [];
let translation: string[] | null = null;
let index = -1;
let mode: Mode = "both";
let visible = false;
let editing = false;
let toastTimer: number | undefined;

function applyMode() {
  root.classList.remove("m-orig", "m-tr", "m-both");
  root.classList.add(translation ? MODE_CLASS[mode] : "m-orig");
}

function render() {
  const src = lines.length ? lines : editing ? SAMPLE : [];
  const nodes = src.map((text, i) => {
    const ln = document.createElement("div");
    ln.className = "ln";
    if (!text.trim()) {
      ln.classList.add("gap");
      ln.textContent = "• • •";
      return ln;
    }
    const tr = translation?.[i] ?? "";
    if (!tr) ln.classList.add("no-tr");
    const t = document.createElement("span");
    t.className = "t";
    t.textContent = tr || text;
    const o = document.createElement("span");
    o.className = "o";
    o.textContent = text;
    ln.append(t, o);
    return ln;
  });
  track.replaceChildren(...nodes);
  applyMode();
  focus();
}

function focus() {
  const els = track.children;
  const cur = lines.length ? index : editing ? 1 : -1;
  for (let i = 0; i < els.length; i++) els[i].classList.toggle("cur", i === cur);
  const el = els[Math.max(cur, 0)] as HTMLElement | undefined;
  if (!el) {
    track.style.transform = "";
    return;
  }
  const ty = viewport.clientHeight / 2 - (el.offsetTop + el.offsetHeight / 2);
  track.style.transform = `translateY(${ty}px)`;
}

function updateVisibility() {
  root.classList.toggle("hidden", !visible && !editing);
}

function applyAppearance(a: Appearance) {
  const s = root.style;
  s.setProperty("--ov-font", FONT_STACKS[a.font] ?? FONT_STACKS.system);
  s.setProperty("--ov-color", a.text_color);
  s.setProperty("--ov-scale", String(a.size));
  s.setProperty("--ov-bg", a.bg_color ? hexToRgba(a.bg_color, a.bg_opacity) : "transparent");
  root.classList.toggle("has-bg", !!a.bg_color);
  requestAnimationFrame(focus);
}

function showToast(text: string) {
  toast.textContent = text;
  toast.classList.add("show");
  window.clearTimeout(toastTimer);
  toastTimer = window.setTimeout(() => toast.classList.remove("show"), 1000);
}

root.addEventListener("mousedown", (e) => {
  if (editing && e.button === 0) void getCurrentWindow().startDragging();
});

new ResizeObserver(() => focus()).observe(viewport);
document.fonts?.ready.then(() => focus());

async function main() {
  await listen<{ lines: string[] }>("lyrics-loaded", (e) => {
    lines = e.payload.lines;
    translation = null;
    index = -1;
    render();
  });
  await listen<{ lines: string[] }>("translation-loaded", (e) => {
    translation = e.payload.lines.length ? e.payload.lines : null;
    render();
  });
  await listen<{ index: number }>("line-changed", (e) => {
    index = e.payload.index;
    focus();
  });
  await listen("hide", () => {
    visible = false;
    updateVisibility();
  });
  await listen("show", () => {
    visible = true;
    updateVisibility();
  });
  await listen<Appearance>("appearance-changed", (e) => applyAppearance(e.payload));
  await listen<{ mode: Mode }>("mode-changed", (e) => {
    mode = e.payload.mode;
    applyMode();
    requestAnimationFrame(focus);
  });
  await listen<{ offset_ms: number }>("offset-changed", (e) => {
    const v = e.payload.offset_ms;
    showToast(`offset ${v > 0 ? "+" : ""}${v} ms`);
  });
  await listen<{ on: boolean }>("edit-mode", (e) => {
    editing = e.payload.on;
    root.classList.toggle("edit", editing);
    render();
    updateVisibility();
  });

  const init = await invoke<OverlayInit>("get_overlay_init");
  applyAppearance(init.appearance);
  mode = init.mode;
  lines = init.snapshot.lines;
  translation = init.snapshot.translation;
  index = init.snapshot.index;
  visible = init.snapshot.visible;
  editing = init.edit;
  root.classList.toggle("edit", editing);
  render();
  updateVisibility();
}

void main();
```

- [ ] **Step 5: Verificar build**

Run: `npm run build`
Expected: `tsc` sem erros; `dist/index.html` gerado e `dist/assets/` com arquivos `.woff2`.

- [ ] **Step 6: Verificação manual (macOS)**

Run: `npm run tauri dev`, com o Spotify tocando uma faixa que tenha letra sincronizada.
Expected:
- overlay aparece centralizado ~120 px acima da borda inferior, fundo transparente, 3 linhas com a atual em destaque;
- a letra rola suavemente a cada troca de linha; pausa instrumental mostra `• • •`;
- clicar "através" do overlay funciona (ex.: clicar num ícone embaixo dele);
- `Cmd+Shift+L`: borda tracejada, arrastar move a janela; `Cmd+Shift+L` de novo salva; reiniciar o app mantém a posição;
- `Cmd+Shift+→`: aviso "offset +250 ms" por 1 s; a letra adianta;
- pausar o Spotify esconde a letra; retomar mostra de novo;
- trocar de faixa esconde e mostra a letra nova.

- [ ] **Step 7: Commit**

```bash
git add src package.json package-lock.json
git commit -m "feat: overlay com rolagem da letra, modos e aparência

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 16: Janela de Preferências

**Files:**
- Create: `src/prefs/style.css`
- Modify: `src/prefs.html`, `src/prefs/main.ts`

**Interfaces:**
- Consumes: comandos `get_settings`, `set_appearance { appearance }`, `set_translation { translation }`, `set_deepl_key { key }`, `get_deepl_usage`, `open_link { link: "support" | "deepl_signup" | "deepl_keys" }`; eventos `mode-changed`, `deepl-status`; `FONT_STACKS`, `FONT_LABELS`, `DEFAULT_APPEARANCE`, tipos da Task 15.
- Produces: janela de Preferências completa.

- [ ] **Step 1: HTML**

`src/prefs.html`:
```html
<!doctype html>
<html lang="pt-BR">
  <head>
    <meta charset="UTF-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1" />
    <title>Preferências</title>
  </head>
  <body>
    <main>
      <section>
        <h2>Aparência</h2>
        <div class="field">
          <span class="lbl">Fonte</span>
          <div class="fonts" id="fonts"></div>
        </div>
        <div class="field">
          <span class="lbl">Tamanho</span>
          <div class="seg" id="sizes">
            <button data-size="0.8">Pequeno</button>
            <button data-size="1">Médio</button>
            <button data-size="1.25">Grande</button>
            <button data-size="1.5">Enorme</button>
          </div>
        </div>
        <div class="field">
          <span class="lbl">Cor do texto</span>
          <div class="sw" id="txt-sw">
            <button data-c="#ffffff" title="Branco"></button>
            <button data-c="#ffe066" title="Amarelo"></button>
            <button data-c="#8ce99a" title="Verde"></button>
            <button data-c="#74c0fc" title="Azul"></button>
            <button data-c="#343a40" title="Grafite"></button>
            <input type="color" id="txt-pick" title="Outra cor" />
          </div>
        </div>
        <div class="field">
          <span class="lbl">Fundo</span>
          <div class="sw" id="bg-sw">
            <button data-c="" class="none" title="Sem fundo">∅</button>
            <button data-c="#000000" title="Preto"></button>
            <button data-c="#0b1d3a" title="Azul-noite"></button>
            <button data-c="#ffffff" title="Branco"></button>
            <input type="color" id="bg-pick" title="Outra cor" />
          </div>
          <label class="range">Opacidade <input type="range" id="bg-op" min="10" max="100" step="5" /> <output id="bg-opv"></output></label>
        </div>
      </section>

      <section>
        <h2>Tradução</h2>
        <div class="field">
          <label class="lbl" for="key">Chave da API DeepL</label>
          <input type="password" id="key" placeholder="cole sua chave aqui" autocomplete="off" spellcheck="false" />
          <small id="key-status"></small>
        </div>
        <div class="field">
          <label class="lbl" for="lang">Traduzir para</label>
          <select id="lang">
            <option value="PT-BR">Português (Brasil)</option>
            <option value="EN-US">Inglês (EUA)</option>
            <option value="ES">Espanhol</option>
          </select>
        </div>
        <div class="field row">
          <button class="btn primary" id="deepl-signup">Criar chave grátis no DeepL</button>
          <a href="#" id="deepl-keys">Já tenho conta: ver minhas chaves</a>
        </div>
        <p class="note" id="usage"></p>
        <p class="warn" id="warn" hidden></p>
      </section>
    </main>
    <footer>
      <button class="bmc" id="bmc" title="Buy me a coffee"><img id="bmc-img" alt="Buy me a coffee" /></button>
      <button class="btn" id="reset">Restaurar padrão</button>
    </footer>
    <script type="module" src="./prefs/main.ts"></script>
  </body>
</html>
```

- [ ] **Step 2: CSS**

`src/prefs/style.css`:
```css
:root {
  --bg: #f6f6f4; --surface: #fff; --ink: #1d1d1f; --muted: #6b6b70; --line: #deded9; --accent: #2f6fed; --warn: #b3261e;
  color-scheme: light dark;
}
@media (prefers-color-scheme: dark) {
  :root { --bg: #1e1e20; --surface: #2a2a2d; --ink: #f2f2f2; --muted: #a0a0a6; --line: #3a3a3e; --accent: #6d9bff; --warn: #ff8a80; }
}
* { box-sizing: border-box; }
html, body { margin: 0; height: 100%; }
body { display: flex; flex-direction: column; background: var(--bg); color: var(--ink); font: 13px/1.4 -apple-system, "Segoe UI", system-ui, sans-serif; }
main { flex: 1; overflow-y: auto; padding: 16px 20px; }
section + section { margin-top: 18px; padding-top: 14px; border-top: 1px solid var(--line); }
h2 { font-size: 13px; text-transform: uppercase; letter-spacing: 0.06em; color: var(--muted); margin: 0 0 10px; }
.field { margin-bottom: 12px; display: flex; flex-direction: column; gap: 6px; }
.field.row { flex-direction: row; align-items: center; gap: 14px; }
.lbl { font-weight: 600; }
.fonts { display: grid; grid-template-columns: repeat(5, 1fr); gap: 6px; }
.fonts button { display: flex; flex-direction: column; align-items: center; gap: 2px; padding: 6px 4px; background: var(--surface); border: 1px solid var(--line); border-radius: 8px; color: var(--ink); cursor: pointer; }
.fonts button b { font-size: 20px; }
.fonts button small { color: var(--muted); font-size: 11px; }
.seg { display: inline-flex; width: fit-content; background: var(--surface); border: 1px solid var(--line); border-radius: 8px; padding: 3px; }
.seg button { border: 0; background: transparent; color: var(--muted); padding: 5px 12px; border-radius: 6px; cursor: pointer; font: inherit; }
button[aria-pressed="true"] { outline: 2px solid var(--accent); outline-offset: 1px; }
.seg button[aria-pressed="true"] { outline: none; background: var(--ink); color: var(--bg); }
.sw { display: flex; align-items: center; gap: 8px; }
.sw button { width: 24px; height: 24px; border-radius: 50%; border: 1px solid var(--line); cursor: pointer; padding: 0; }
.sw button.none { background: var(--surface); color: var(--muted); font-size: 12px; }
.sw input[type="color"] { width: 28px; height: 26px; border: 0; background: none; padding: 0; cursor: pointer; }
.range { display: flex; align-items: center; gap: 8px; color: var(--muted); }
input[type="password"], select { font: inherit; padding: 6px 8px; border: 1px solid var(--line); border-radius: 6px; background: var(--surface); color: var(--ink); }
.btn { font: inherit; padding: 6px 12px; border-radius: 6px; border: 1px solid var(--line); background: var(--surface); color: var(--ink); cursor: pointer; }
.btn.primary { background: var(--accent); border-color: var(--accent); color: #fff; }
a { color: var(--accent); }
.note { color: var(--muted); margin: 4px 0; }
.warn { color: var(--warn); margin: 4px 0; }
small { color: var(--muted); }
footer { display: flex; justify-content: space-between; align-items: center; padding: 10px 20px; border-top: 1px solid var(--line); background: var(--surface); }
.bmc { border: 0; background: none; padding: 0; cursor: pointer; }
.bmc img { height: 34px; display: block; }
```

- [ ] **Step 3: Lógica**

`src/prefs/main.ts`:
```ts
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import bmcButton from "../assets/bmc/bmc-button.svg";
import { FONT_LABELS, FONT_STACKS } from "../shared/fonts";
import {
  DEFAULT_APPEARANCE,
  type Appearance,
  type DeepLStatus,
  type FontId,
  type Mode,
  type SettingsView,
  type TargetLang,
  type TranslationCfg,
  type Usage,
} from "../shared/types";
import "./style.css";

const $ = <T extends HTMLElement>(id: string) => document.getElementById(id) as T;

let appearance: Appearance = { ...DEFAULT_APPEARANCE };
let translation: TranslationCfg = { mode: "both", target_lang: "PT-BR" };
let hasKey = false;
let saveTimer: number | undefined;

function saveAppearance() {
  window.clearTimeout(saveTimer);
  saveTimer = window.setTimeout(() => void invoke("set_appearance", { appearance }), 120);
}

function setAppearance(patch: Partial<Appearance>) {
  appearance = { ...appearance, ...patch };
  renderAppearance();
  saveAppearance();
}

function buildFonts() {
  const box = $("fonts");
  box.replaceChildren(
    ...(Object.keys(FONT_LABELS) as FontId[]).map((id) => {
      const b = document.createElement("button");
      b.dataset.font = id;
      const aa = document.createElement("b");
      aa.textContent = "Aa";
      aa.style.fontFamily = FONT_STACKS[id];
      const label = document.createElement("small");
      label.textContent = FONT_LABELS[id];
      b.append(aa, label);
      b.onclick = () => setAppearance({ font: id });
      return b;
    }),
  );
}

function renderAppearance() {
  document.querySelectorAll<HTMLButtonElement>("#fonts button").forEach((b) =>
    b.setAttribute("aria-pressed", String(b.dataset.font === appearance.font)),
  );
  document.querySelectorAll<HTMLButtonElement>("#sizes button").forEach((b) =>
    b.setAttribute("aria-pressed", String(Number(b.dataset.size) === appearance.size)),
  );
  document.querySelectorAll<HTMLButtonElement>("#txt-sw button").forEach((b) => {
    b.style.background = b.dataset.c!;
    b.setAttribute("aria-pressed", String(b.dataset.c === appearance.text_color));
  });
  document.querySelectorAll<HTMLButtonElement>("#bg-sw button").forEach((b) => {
    if (b.dataset.c) b.style.background = b.dataset.c;
    b.setAttribute("aria-pressed", String((b.dataset.c || null) === appearance.bg_color));
  });
  $<HTMLInputElement>("txt-pick").value = appearance.text_color;
  if (appearance.bg_color) $<HTMLInputElement>("bg-pick").value = appearance.bg_color;
  const op = $<HTMLInputElement>("bg-op");
  op.value = String(appearance.bg_opacity);
  op.disabled = !appearance.bg_color;
  $("bg-opv").textContent = `${appearance.bg_opacity}%`;
}

function renderStatus(status: DeepLStatus) {
  const warn = $("warn");
  const msg: Record<DeepLStatus, string> = {
    ok: "",
    invalid_key: "Chave DeepL inválida. Mostrando só a letra original até você trocar a chave.",
    quota_exceeded: "Cota grátis do DeepL deste mês esgotada. Mostrando só a letra original.",
  };
  warn.textContent = msg[status];
  warn.hidden = status === "ok";
}

async function loadUsage() {
  const el = $("usage");
  if (!hasKey) {
    el.textContent = "Sem chave: a letra aparece só no idioma original.";
    return;
  }
  try {
    const u = await invoke<Usage>("get_deepl_usage");
    const fmt = new Intl.NumberFormat("pt-BR");
    el.textContent = `Uso do mês: ${fmt.format(u.character_count)} / ${fmt.format(u.character_limit)} caracteres`;
  } catch {
    el.textContent = "Não foi possível consultar o uso do DeepL agora.";
  }
}

function wire() {
  document.querySelectorAll<HTMLButtonElement>("#sizes button").forEach((b) => {
    b.onclick = () => setAppearance({ size: Number(b.dataset.size) });
  });
  document.querySelectorAll<HTMLButtonElement>("#txt-sw button").forEach((b) => {
    b.onclick = () => setAppearance({ text_color: b.dataset.c! });
  });
  document.querySelectorAll<HTMLButtonElement>("#bg-sw button").forEach((b) => {
    b.onclick = () => setAppearance({ bg_color: b.dataset.c || null });
  });
  $<HTMLInputElement>("txt-pick").oninput = (e) => setAppearance({ text_color: (e.target as HTMLInputElement).value });
  $<HTMLInputElement>("bg-pick").oninput = (e) => setAppearance({ bg_color: (e.target as HTMLInputElement).value });
  $<HTMLInputElement>("bg-op").oninput = (e) => setAppearance({ bg_opacity: Number((e.target as HTMLInputElement).value) });
  $("reset").onclick = () => setAppearance({ ...DEFAULT_APPEARANCE });

  const key = $<HTMLInputElement>("key");
  key.onchange = async () => {
    const status = $("key-status");
    try {
      await invoke("set_deepl_key", { key: key.value });
      hasKey = key.value.trim() !== "";
      status.textContent = hasKey ? "Chave salva no cofre do sistema." : "Chave removida.";
      key.value = "";
      key.placeholder = hasKey ? "•••••••• (salva)" : "cole sua chave aqui";
      renderStatus("ok");
      void loadUsage();
    } catch (err) {
      status.textContent = `Não foi possível salvar a chave: ${err}`;
    }
  };

  $<HTMLSelectElement>("lang").onchange = (e) => {
    translation = { ...translation, target_lang: (e.target as HTMLSelectElement).value as TargetLang };
    void invoke("set_translation", { translation });
  };

  $("deepl-signup").onclick = () => void invoke("open_link", { link: "deepl_signup" });
  $("deepl-keys").onclick = (e) => {
    e.preventDefault();
    void invoke("open_link", { link: "deepl_keys" });
  };
  $("bmc").onclick = () => void invoke("open_link", { link: "support" });
  $<HTMLImageElement>("bmc-img").src = bmcButton;
}

async function main() {
  buildFonts();
  wire();
  await listen<{ mode: Mode; target_lang: TargetLang }>("mode-changed", (e) => {
    translation = { mode: e.payload.mode, target_lang: e.payload.target_lang };
    $<HTMLSelectElement>("lang").value = translation.target_lang;
  });
  await listen<DeepLStatus>("deepl-status", (e) => renderStatus(e.payload));

  const s = await invoke<SettingsView>("get_settings");
  appearance = s.appearance;
  translation = s.translation;
  hasKey = s.has_key;
  $<HTMLSelectElement>("lang").value = translation.target_lang;
  $<HTMLInputElement>("key").placeholder = hasKey ? "•••••••• (salva)" : "cole sua chave aqui";
  renderAppearance();
  renderStatus(s.deepl_status);
  void loadUsage();
}

void main();
```

- [ ] **Step 4: Verificar build**

Run: `npm run build`
Expected: `tsc` sem erros; `dist/prefs.html` gerado; o SVG do BMC aparece em `dist/assets/`.

- [ ] **Step 5: Verificação manual (macOS)**

Run: `npm run tauri dev` → menu do ícone → Preferências… (ou `Cmd+,` com o menu aberto).
Expected:
- janela "Preferências" ~560×440; abrir de novo só foca a existente;
- trocar fonte (incluindo Manuscrita com letra em japonês), tamanho, cor do texto, fundo e opacidade muda o overlay na hora; o fundo aparece só atrás da linha em destaque;
- Tamanho "Enorme" aumenta a janela do overlay mantendo o centro; "Restaurar padrão" volta tudo;
- colar uma chave DeepL e sair do campo → "Chave salva no cofre do sistema."; o uso do mês aparece; a tradução chega no overlay (faixa em outro idioma);
- chave inválida → aviso vermelho e o overlay mostra só o original;
- "Criar chave grátis no DeepL", "ver minhas chaves" e o botão do BMC abrem as URLs corretas no navegador;
- o `config.json` (em `~/Library/Application Support/dev.luizfbalves.lyricoverlay/`) **não** contém a chave.

- [ ] **Step 6: Commit**

```bash
git add src
git commit -m "feat: janela de Preferências com aparência, tradução e BMC

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 17: Validação ponta a ponta (macOS e Windows)

**Files:** nenhum código novo; correções que surgirem viram commits `fix:`.

- [ ] **Step 1: Suíte completa**

Run: `cargo test --manifest-path src-tauri/Cargo.toml && cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings && npm run build`
Expected: tudo verde.

- [ ] **Step 2: Build de release no macOS**

Run: `npm run tauri build`
Expected: `.app` em `src-tauri/target/release/bundle/macos/`. Abrir o app (liberar no Gatekeeper com clique direito → Abrir). No primeiro uso, o macOS pede permissão de Automação para o Spotify com o texto do `Info.plist`; aceitar.

- [ ] **Step 3: Checklist macOS (app de release)**

- [ ] Critério de sucesso: a linha acompanha a música com atraso < ~300 ms (comparar com o player de letras do próprio Spotify).
- [ ] Seek no Spotify: a letra se ajusta em até ~1 s.
- [ ] Spotify fechado → nada na tela, menu mostra "Nada tocando"; reabrir e tocar → volta.
- [ ] Anúncio/podcast → overlay escondido.
- [ ] Negar a Automação (Ajustes → Privacidade → Automação) → app não trava; conceder de novo → volta a funcionar.
- [ ] Offset por faixa persiste após reiniciar o app; "Resetar offset desta faixa" zera.
- [ ] Posição salva num segundo monitor; desconectar o monitor e reabrir → volta para a posição padrão.
- [ ] Overlay visível em todos os Spaces e sobre apps em tela cheia.
- [ ] Modos pelo menu: Só original / Só tradução / Original + tradução (tradução maior, em cima).
- [ ] Sem rede: nada no overlay, sem travar; voltar a rede e trocar de faixa → funciona.

- [ ] **Step 4: Windows**

Em uma máquina Windows 10/11 com Node, Rust e WebView2:
```bash
npm install
npm run tauri build
```
Repetir o checklist do Step 3 trocando: ícone na bandeja do sistema; atalhos com `Ctrl+Shift`; sem pedido de Automação; conferir que a posição lida pelo SMTC acompanha a música (se ficar atrasada/adiantada de forma constante, registrar o desvio e ajustar a correção de `LastUpdatedTime` em `player/windows.rs` com um commit `fix:`).

- [ ] **Step 5: Commit final (se houve ajustes)**

```bash
git add -A
git commit -m "fix: ajustes da validação ponta a ponta

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```
