use std::fmt;

pub mod macos;

#[cfg(windows)]
pub mod windows;

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
