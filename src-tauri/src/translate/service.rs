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

#[derive(Clone)]
pub struct TranslateSettings {
    pub mode: Mode,
    pub target: TargetLang,
    pub key: Option<String>,
}

impl std::fmt::Debug for TranslateSettings {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TranslateSettings")
            .field("mode", &self.mode)
            .field("target", &self.target)
            .field("key", &self.key.as_ref().map(|_| "***"))
            .finish()
    }
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
        if s.mode == Mode::Original {
            return None;
        }
        let target = s.target.code();
        if let Some(hit) = self.cache.get(key, target) {
            // Um hit cujo número de linhas não bate com a letra atual (ex.: LRC reemitido/
            // reformatado para a mesma faixa) é tratado como miss: segue para retraduzir em
            // vez de devolver linhas desalinhadas.
            let stale = hit.lines.as_ref().is_some_and(|l| l.len() != lines.len());
            if !stale {
                return hit.lines;
            }
        }
        // O gate de status só protege a chamada de rede: um hit de cache não depende da chave.
        if self.status() != DeepLStatus::Ok {
            return None;
        }
        let api_key = s.key?;
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

#[async_trait::async_trait]
impl crate::sync::runtime::TrackTranslator for TranslationService {
    async fn translate_track(&self, key: &TrackKey, lines: &[String]) -> Option<Vec<String>> {
        TranslationService::translate_track(self, key, lines).await
    }

    fn current_target(&self) -> String {
        self.settings().target.code().to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{Mode, TargetLang};
    use serde_json::{json, Value};
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{Arc, Mutex};
    use wiremock::matchers::path;
    use wiremock::{Mock, MockServer, Request, Respond, ResponseTemplate};

    /// 1ª chamada responde OK (como `Echo`), as seguintes respondem 403 — simula a chave
    /// virando inválida depois de uma tradução bem-sucedida anterior.
    struct FirstOkThen403 {
        calls: AtomicUsize,
    }
    impl FirstOkThen403 {
        fn new() -> Self {
            Self { calls: AtomicUsize::new(0) }
        }
    }
    impl Respond for FirstOkThen403 {
        fn respond(&self, req: &Request) -> ResponseTemplate {
            if self.calls.fetch_add(1, Ordering::SeqCst) > 0 {
                return ResponseTemplate::new(403);
            }
            let v: Value = serde_json::from_slice(&req.body).unwrap();
            let items: Vec<Value> = v["text"]
                .as_array()
                .unwrap()
                .iter()
                .map(|t| json!({"detected_source_language": "JA", "text": format!("tr:{}", t.as_str().unwrap())}))
                .collect();
            ResponseTemplate::new(200).set_body_json(json!({ "translations": items }))
        }
    }

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

    #[tokio::test]
    async fn cached_translation_survives_invalid_key_status() {
        let s = MockServer::start().await;
        Mock::given(path("/v2/translate")).respond_with(FirstOkThen403::new()).expect(2).mount(&s).await;
        let f = fixture(&s, Mode::Both, Some("k"));
        let want = Some(vec!["tr:um".to_string(), "".to_string(), "tr:dois".to_string()]);

        // 1ª requisição: popula o cache da faixa A com sucesso.
        assert_eq!(f.svc.translate_track(&key(), &lines()).await, want);

        // 2ª requisição: faixa B (fora do cache) recebe 403 e deixa o status InvalidKey.
        let other = TrackKey { artist: "Outra Banda".into(), title: "Outra Canção".into(), duration_s: 200 };
        assert_eq!(f.svc.translate_track(&other, &lines()).await, None);
        assert_eq!(f.svc.status(), DeepLStatus::InvalidKey);

        // faixa A já está em cache: deve devolver o hit sem nova requisição, mesmo com a chave inválida.
        assert_eq!(f.svc.translate_track(&key(), &lines()).await, want);
    }

    #[tokio::test]
    async fn cache_hit_with_mismatched_line_count_is_retranslated() {
        let s = MockServer::start().await;
        Mock::given(path("/v2/translate")).respond_with(Echo("JA")).expect(2).mount(&s).await;
        let f = fixture(&s, Mode::Both, Some("k"));
        let want = Some(vec!["tr:um".to_string(), "".to_string(), "tr:dois".to_string()]);
        assert_eq!(f.svc.translate_track(&key(), &lines()).await, want);

        // A letra "atual" da mesma faixa agora tem uma quantidade diferente de linhas (ex.:
        // LRC reemitido) — o hit de cache antigo (3 linhas) não pode ser reaproveitado.
        let new_lines = vec!["um".to_string(), "dois".to_string()];
        let want2 = Some(vec!["tr:um".to_string(), "tr:dois".to_string()]);
        assert_eq!(f.svc.translate_track(&key(), &new_lines).await, want2);
    }

    #[test]
    fn status_serializes_snake_case() {
        assert_eq!(serde_json::to_value(DeepLStatus::InvalidKey).unwrap(), "invalid_key");
        assert_eq!(serde_json::to_value(DeepLStatus::QuotaExceeded).unwrap(), "quota_exceeded");
    }

    #[test]
    fn debug_redacts_key() {
        let s = TranslateSettings { mode: Mode::Both, target: TargetLang::PtBr, key: Some("segredo-super-secreto".into()) };
        let out = format!("{s:?}");
        assert!(!out.contains("segredo-super-secreto"), "chave vazou no Debug: {out}");
        assert!(out.contains("***"));
    }
}
