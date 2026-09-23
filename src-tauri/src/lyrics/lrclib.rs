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
            .user_agent(concat!("verso/", env!("CARGO_PKG_VERSION"), " (uso pessoal)"))
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
