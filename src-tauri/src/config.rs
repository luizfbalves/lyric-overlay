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
        Ok(s) => match serde_json::from_str::<Config>(&s) {
            Ok(c) => c.normalized(),
            Err(e) => {
                eprintln!("config corrompida em {}: {e}", path.display());
                Config::default()
            }
        },
        Err(e) => {
            if e.kind() != std::io::ErrorKind::NotFound {
                eprintln!("ler config em {}: {e}", path.display());
            }
            Config::default()
        }
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
        let mut offsets = HashMap::new();
        offsets.insert("Banda Fictícia|Canção Teste|180".into(), 250);
        let c = Config {
            window: Some(WindowPos { x: -300, y: 40 }),
            offsets,
            appearance: Appearance {
                font: FontId::Handwritten,
                size: 1.25,
                text_color: "#ffe066".into(),
                bg_color: Some("#0b1d3a".into()),
                bg_opacity: 80,
            },
            translation: TranslationCfg { mode: Mode::Translated, target_lang: TargetLang::Es },
        };
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
