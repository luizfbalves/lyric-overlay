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
