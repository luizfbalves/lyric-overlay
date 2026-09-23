use super::{TranslateError, Translated, Translator};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Mesmo limite do proxy (proxy/src/index.js): acima disso ele responde 400.
const BATCH: usize = 400;

#[derive(Serialize)]
struct Req<'a> {
    to: &'a str,
    lines: Vec<&'a str>,
}

#[derive(Deserialize)]
struct Resp {
    lines: Vec<String>,
    source: String,
}

/// Cliente do proxy de tradução do Verso (Cloudflare Worker na frente da Azure Translator).
pub struct ProxyTranslator {
    http: reqwest::Client,
    base: String,
}

fn net(e: reqwest::Error) -> TranslateError {
    TranslateError::Network(e.to_string())
}

impl ProxyTranslator {
    pub fn new(base: &str, timeout: Duration) -> Self {
        let http = reqwest::Client::builder()
            .timeout(timeout)
            .user_agent(concat!("Verso/", env!("CARGO_PKG_VERSION")))
            .build()
            .expect("cliente HTTP");
        Self { http, base: base.trim_end_matches('/').to_string() }
    }
}

#[async_trait]
impl Translator for ProxyTranslator {
    /// `target` é o código de idioma da Azure (ex.: "pt").
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
            let req = Req { to: target, lines: chunk.iter().map(|&i| lines[i].as_str()).collect() };
            let resp = self
                .http
                .post(format!("{}/v1/translate", self.base))
                .json(&req)
                .send()
                .await
                .map_err(net)?;
            match resp.status().as_u16() {
                503 => return Err(TranslateError::QuotaExceeded),
                // 429 (limite por IP) e 5xx passageiros: não guarda nada, tenta de novo na próxima faixa.
                s if !(200..300).contains(&s) => return Err(TranslateError::Network(format!("HTTP {s}"))),
                _ => {}
            }
            let body: Resp = resp.json().await.map_err(net)?;
            if body.lines.len() != chunk.len() {
                return Err(TranslateError::Unexpected(format!(
                    "proxy devolveu {} de {} linhas",
                    body.lines.len(),
                    chunk.len()
                )));
            }
            if source_lang.is_empty() {
                source_lang = body.source;
            }
            for (&i, text) in chunk.iter().zip(body.lines) {
                out[i] = text;
            }
        }
        Ok(Translated { lines: out, source_lang })
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use serde_json::{json, Value};
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, Request, Respond, ResponseTemplate};

    /// Responde "tr:<texto>" para cada linha recebida, com idioma de origem fixo.
    pub(crate) struct Echo(pub &'static str);
    impl Respond for Echo {
        fn respond(&self, req: &Request) -> ResponseTemplate {
            let v: Value = serde_json::from_slice(&req.body).unwrap();
            let lines: Vec<String> =
                v["lines"].as_array().unwrap().iter().map(|t| format!("tr:{}", t.as_str().unwrap())).collect();
            ResponseTemplate::new(200).set_body_json(json!({ "lines": lines, "source": self.0 }))
        }
    }

    fn client(s: &MockServer) -> ProxyTranslator {
        ProxyTranslator::new(&s.uri(), Duration::from_secs(2))
    }

    fn strs(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    #[tokio::test]
    async fn preserves_empty_lines_and_indices() {
        let s = MockServer::start().await;
        Mock::given(method("POST")).and(path("/v1/translate")).respond_with(Echo("ja")).expect(1).mount(&s).await;
        let t = client(&s).translate(&strs(&["um", "", "  ", "dois"]), "pt").await.unwrap();
        assert_eq!(t.lines, strs(&["tr:um", "", "", "tr:dois"]));
        assert_eq!(t.source_lang, "ja");
        let req = &s.received_requests().await.unwrap()[0];
        let body: Value = serde_json::from_slice(&req.body).unwrap();
        assert_eq!(body, json!({ "to": "pt", "lines": ["um", "dois"] }));
        assert!(req.headers["user-agent"].to_str().unwrap().starts_with("Verso/"));
    }

    #[tokio::test]
    async fn splits_into_batches() {
        let s = MockServer::start().await;
        Mock::given(path("/v1/translate")).respond_with(Echo("ja")).expect(2).mount(&s).await;
        let input: Vec<String> = (0..450).map(|i| format!("linha {i}")).collect();
        let t = client(&s).translate(&input, "pt").await.unwrap();
        assert_eq!(t.lines.len(), 450);
        assert_eq!(t.lines[449], "tr:linha 449");
    }

    #[tokio::test]
    async fn all_empty_makes_no_request() {
        let s = MockServer::start().await;
        Mock::given(path("/v1/translate")).respond_with(Echo("ja")).expect(0).mount(&s).await;
        let t = client(&s).translate(&strs(&["", ""]), "pt").await.unwrap();
        assert_eq!(t.lines, strs(&["", ""]));
    }

    #[tokio::test]
    async fn maps_status_codes() {
        for (code, want_quota) in [(503, true), (429, false), (502, false)] {
            let s = MockServer::start().await;
            Mock::given(path("/v1/translate")).respond_with(ResponseTemplate::new(code)).mount(&s).await;
            let r = client(&s).translate(&strs(&["a"]), "pt").await;
            if want_quota {
                assert_eq!(r, Err(TranslateError::QuotaExceeded));
            } else {
                assert!(matches!(r, Err(TranslateError::Network(_))), "{code}: {r:?}");
            }
        }
    }

    #[tokio::test]
    async fn timeout_is_network_error() {
        let s = MockServer::start().await;
        Mock::given(path("/v1/translate"))
            .respond_with(ResponseTemplate::new(200).set_delay(Duration::from_millis(500)))
            .mount(&s)
            .await;
        let c = ProxyTranslator::new(&s.uri(), Duration::from_millis(100));
        assert!(matches!(c.translate(&strs(&["a"]), "pt").await, Err(TranslateError::Network(_))));
    }

    #[tokio::test]
    async fn count_mismatch_is_error() {
        let s = MockServer::start().await;
        Mock::given(path("/v1/translate"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({"lines": ["só uma"], "source": "ja"})))
            .mount(&s)
            .await;
        let r = client(&s).translate(&strs(&["a", "b"]), "pt").await;
        assert!(matches!(r, Err(TranslateError::Unexpected(_))));
    }
}
