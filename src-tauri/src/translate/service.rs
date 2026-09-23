use super::cache::{CachedTranslation, DiskCache};
use super::proxy::ProxyTranslator;
use super::{same_language, TranslateError, Translator};
use crate::config::{Mode, TargetLang};
use crate::player::TrackKey;
use serde::Serialize;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TranslateStatus {
    Ok,
    QuotaExceeded,
}

/// Com a cota do mês esgotada no proxy, espera esse tempo antes de tentar de novo.
const QUOTA_RETRY: Duration = Duration::from_secs(3600);

#[derive(Debug, Clone)]
pub struct TranslateSettings {
    pub mode: Mode,
    pub target: TargetLang,
}

pub struct TranslationService {
    settings: Mutex<TranslateSettings>,
    status: Mutex<TranslateStatus>,
    retry_at: Mutex<Option<Instant>>,
    quota_retry: Duration,
    cache: DiskCache,
    client: ProxyTranslator,
    on_status: Box<dyn Fn(TranslateStatus) + Send + Sync>,
}

impl TranslationService {
    pub fn new(
        cache_dir: PathBuf,
        settings: TranslateSettings,
        proxy_url: &str,
        timeout: Duration,
        on_status: Box<dyn Fn(TranslateStatus) + Send + Sync>,
    ) -> Self {
        Self {
            settings: Mutex::new(settings),
            status: Mutex::new(TranslateStatus::Ok),
            retry_at: Mutex::new(None),
            quota_retry: QUOTA_RETRY,
            cache: DiskCache::new(cache_dir),
            client: ProxyTranslator::new(proxy_url, timeout),
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

    pub fn status(&self) -> TranslateStatus {
        *self.status.lock().unwrap()
    }

    fn set_status(&self, s: TranslateStatus) {
        let changed = std::mem::replace(&mut *self.status.lock().unwrap(), s) != s;
        if changed {
            (self.on_status)(s);
        }
    }

    /// Cota esgotada bloqueia a rede até `retry_at`; depois disso deixa uma tentativa passar.
    fn blocked(&self) -> bool {
        self.retry_at.lock().unwrap().is_some_and(|t| Instant::now() < t)
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
        // O bloqueio da cota só protege a chamada de rede: um hit de cache continua valendo.
        if self.blocked() {
            return None;
        }
        match self.client.translate(lines, s.target.azure_code()).await {
            Ok(t) => {
                *self.retry_at.lock().unwrap() = None;
                self.set_status(TranslateStatus::Ok);
                let entry = CachedTranslation {
                    lines: (!same_language(&t.source_lang, target)).then_some(t.lines),
                    source_lang: t.source_lang,
                };
                if let Err(e) = self.cache.put(key, target, &entry) {
                    eprintln!("cache de tradução: {e}");
                }
                entry.lines
            }
            Err(TranslateError::QuotaExceeded) => {
                *self.retry_at.lock().unwrap() = Some(Instant::now() + self.quota_retry);
                self.set_status(TranslateStatus::QuotaExceeded);
                None
            }
            Err(e) => {
                eprintln!("tradução falhou: {e:?}");
                None
            }
        }
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
    use crate::translate::proxy::tests::Echo;
    use std::sync::{Arc, Mutex};
    use wiremock::matchers::path;
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn key() -> TrackKey {
        TrackKey { artist: "Banda Fictícia".into(), title: "Canção Teste".into(), duration_s: 180 }
    }

    fn lines() -> Vec<String> {
        vec!["um".into(), "".into(), "dois".into()]
    }

    fn want() -> Option<Vec<String>> {
        Some(vec!["tr:um".to_string(), "".to_string(), "tr:dois".to_string()])
    }

    struct Fixture {
        svc: TranslationService,
        statuses: Arc<Mutex<Vec<TranslateStatus>>>,
        _dir: tempfile::TempDir,
    }

    fn fixture(server: &MockServer, mode: Mode) -> Fixture {
        let dir = tempfile::tempdir().unwrap();
        let statuses = Arc::new(Mutex::new(Vec::new()));
        let rec = statuses.clone();
        let svc = TranslationService::new(
            dir.path().join("translations"),
            TranslateSettings { mode, target: TargetLang::PtBr },
            &server.uri(),
            Duration::from_millis(300),
            Box::new(move |s| rec.lock().unwrap().push(s)),
        );
        Fixture { svc, statuses, _dir: dir }
    }

    #[tokio::test]
    async fn original_mode_skips() {
        let s = MockServer::start().await;
        Mock::given(path("/v1/translate")).respond_with(Echo("ja")).expect(0).mount(&s).await;
        assert_eq!(fixture(&s, Mode::Original).svc.translate_track(&key(), &lines()).await, None);
    }

    #[tokio::test]
    async fn sends_azure_code() {
        let s = MockServer::start().await;
        Mock::given(path("/v1/translate")).respond_with(Echo("ja")).mount(&s).await;
        fixture(&s, Mode::Both).svc.translate_track(&key(), &lines()).await;
        let body: serde_json::Value = serde_json::from_slice(&s.received_requests().await.unwrap()[0].body).unwrap();
        assert_eq!(body["to"], "pt");
    }

    #[tokio::test]
    async fn translates_once_then_uses_cache() {
        let s = MockServer::start().await;
        Mock::given(path("/v1/translate")).respond_with(Echo("ja")).expect(1).mount(&s).await;
        let f = fixture(&s, Mode::Both);
        assert_eq!(f.svc.translate_track(&key(), &lines()).await, want());
        assert_eq!(f.svc.translate_track(&key(), &lines()).await, want());
    }

    #[tokio::test]
    async fn same_language_is_cached_as_not_needed() {
        let s = MockServer::start().await;
        Mock::given(path("/v1/translate")).respond_with(Echo("pt")).expect(1).mount(&s).await;
        let f = fixture(&s, Mode::Both);
        assert_eq!(f.svc.translate_track(&key(), &lines()).await, None);
        assert_eq!(f.svc.translate_track(&key(), &lines()).await, None);
    }

    #[tokio::test]
    async fn quota_blocks_network_until_retry_time() {
        let s = MockServer::start().await;
        Mock::given(path("/v1/translate")).respond_with(ResponseTemplate::new(503)).expect(1).mount(&s).await;
        let f = fixture(&s, Mode::Translated);
        assert_eq!(f.svc.translate_track(&key(), &lines()).await, None);
        assert_eq!(f.svc.status(), TranslateStatus::QuotaExceeded);
        assert_eq!(f.svc.translate_track(&key(), &lines()).await, None); // sem nova requisição
    }

    #[tokio::test]
    async fn quota_retries_after_wait_and_recovers() {
        let s = MockServer::start().await;
        Mock::given(path("/v1/translate")).respond_with(ResponseTemplate::new(503)).up_to_n_times(1).mount(&s).await;
        Mock::given(path("/v1/translate")).respond_with(Echo("ja")).mount(&s).await;
        let mut f = fixture(&s, Mode::Both);
        f.svc.quota_retry = Duration::ZERO;
        assert_eq!(f.svc.translate_track(&key(), &lines()).await, None);
        assert_eq!(f.svc.translate_track(&key(), &lines()).await, want());
        assert_eq!(f.svc.status(), TranslateStatus::Ok);
        assert_eq!(*f.statuses.lock().unwrap(), vec![TranslateStatus::QuotaExceeded, TranslateStatus::Ok]);
    }

    #[tokio::test]
    async fn network_errors_are_not_cached() {
        let s = MockServer::start().await;
        Mock::given(path("/v1/translate")).respond_with(ResponseTemplate::new(502)).expect(2).mount(&s).await;
        let f = fixture(&s, Mode::Both);
        assert_eq!(f.svc.translate_track(&key(), &lines()).await, None);
        assert_eq!(f.svc.translate_track(&key(), &lines()).await, None);
        assert_eq!(f.svc.status(), TranslateStatus::Ok);
        assert!(f.statuses.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn cached_translation_survives_quota_block() {
        let s = MockServer::start().await;
        Mock::given(path("/v1/translate")).respond_with(Echo("ja")).up_to_n_times(1).mount(&s).await;
        Mock::given(path("/v1/translate")).respond_with(ResponseTemplate::new(503)).mount(&s).await;
        let f = fixture(&s, Mode::Both);
        assert_eq!(f.svc.translate_track(&key(), &lines()).await, want());

        let other = TrackKey { artist: "Outra Banda".into(), title: "Outra Canção".into(), duration_s: 200 };
        assert_eq!(f.svc.translate_track(&other, &lines()).await, None);
        assert_eq!(f.svc.status(), TranslateStatus::QuotaExceeded);

        // A faixa A já está em cache: devolve o hit mesmo com a cota esgotada.
        assert_eq!(f.svc.translate_track(&key(), &lines()).await, want());
    }

    #[tokio::test]
    async fn cache_hit_with_mismatched_line_count_is_retranslated() {
        let s = MockServer::start().await;
        Mock::given(path("/v1/translate")).respond_with(Echo("ja")).expect(2).mount(&s).await;
        let f = fixture(&s, Mode::Both);
        assert_eq!(f.svc.translate_track(&key(), &lines()).await, want());

        // A letra "atual" da mesma faixa agora tem uma quantidade diferente de linhas (ex.:
        // LRC reemitido) — o hit de cache antigo (3 linhas) não pode ser reaproveitado.
        let new_lines = vec!["um".to_string(), "dois".to_string()];
        let want2 = Some(vec!["tr:um".to_string(), "tr:dois".to_string()]);
        assert_eq!(f.svc.translate_track(&key(), &new_lines).await, want2);
    }

    #[test]
    fn status_serializes_snake_case() {
        assert_eq!(serde_json::to_value(TranslateStatus::QuotaExceeded).unwrap(), "quota_exceeded");
    }
}
