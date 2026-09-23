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

/// Tempo decorrido desde um `DateTime.UniversalTime` do WinRT (ticks de 100 ns desde 1601-01-01 UTC).
pub fn smtc_elapsed_ms(universal_time: i64, now_unix_ms: u64) -> u64 {
    const EPOCH_DIFF_MS: i64 = 11_644_473_600_000;
    let then_unix_ms = universal_time / 10_000 - EPOCH_DIFF_MS;
    (now_unix_ms as i64 - then_unix_ms).max(0) as u64
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

#[cfg(test)]
mod tests {
    use super::*;

    // 2026-01-01T00:00:00Z em ms Unix e em ticks de 100 ns desde 1601.
    const UNIX_MS: u64 = 1_767_225_600_000;
    const WIN_TICKS: i64 = (1_767_225_600_000 + 11_644_473_600_000) * 10_000;

    #[test]
    fn smtc_elapsed() {
        assert_eq!(smtc_elapsed_ms(WIN_TICKS, UNIX_MS), 0);
        assert_eq!(smtc_elapsed_ms(WIN_TICKS, UNIX_MS + 2_500), 2_500);
        // relógio "voltou": nunca negativo
        assert_eq!(smtc_elapsed_ms(WIN_TICKS, UNIX_MS - 1_000), 0);
    }
}
