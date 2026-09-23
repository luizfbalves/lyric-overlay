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
