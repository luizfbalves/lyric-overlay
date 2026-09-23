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
