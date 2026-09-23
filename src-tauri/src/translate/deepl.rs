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
