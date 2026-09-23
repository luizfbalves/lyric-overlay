use super::{smtc_elapsed_ms, NowPlaying, Player, PlayerError};
use ::windows::Media::Control::{
    GlobalSystemMediaTransportControlsSessionManager as Manager,
    GlobalSystemMediaTransportControlsSessionPlaybackStatus as Status,
};
use std::time::{SystemTime, UNIX_EPOCH};

pub struct WinSmtcPlayer;

fn err(e: ::windows::core::Error) -> PlayerError {
    PlayerError(format!("SMTC: {e}"))
}

impl Player for WinSmtcPlayer {
    fn now_playing(&self) -> Result<Option<NowPlaying>, PlayerError> {
        let mgr = Manager::RequestAsync().map_err(err)?.get().map_err(err)?;
        let sessions = mgr.GetSessions().map_err(err)?;
        for s in sessions {
            let id = s.SourceAppUserModelId().map_err(err)?.to_string();
            if !id.to_lowercase().contains("spotify") {
                continue;
            }
            let props = s.TryGetMediaPropertiesAsync().map_err(err)?.get().map_err(err)?;
            let tl = s.GetTimelineProperties().map_err(err)?;
            let status = s.GetPlaybackInfo().map_err(err)?.PlaybackStatus().map_err(err)?;
            let is_playing = status == Status::Playing;
            let duration_ms = (tl.EndTime().map_err(err)?.Duration / 10_000).max(0) as u64;
            let mut position_ms = (tl.Position().map_err(err)?.Duration / 10_000).max(0) as u64;
            if is_playing {
                let now = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0);
                position_ms += smtc_elapsed_ms(tl.LastUpdatedTime().map_err(err)?.UniversalTime, now);
            }
            if duration_ms > 0 {
                position_ms = position_ms.min(duration_ms);
            }
            return Ok(Some(NowPlaying {
                title: props.Title().map_err(err)?.to_string(),
                artist: props.Artist().map_err(err)?.to_string(),
                album: props.AlbumTitle().map_err(err)?.to_string(),
                duration_ms,
                position_ms,
                is_playing,
            }));
        }
        Ok(None)
    }
}
