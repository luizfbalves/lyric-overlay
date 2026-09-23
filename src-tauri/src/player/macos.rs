use super::{NowPlaying, Player, PlayerError};
use std::process::Command;

const SCRIPT: &str = r#"if application "Spotify" is running then
  tell application "Spotify"
    if player state is stopped then return ""
    set sep to (ASCII character 31)
    set t to current track
    return (name of t) & sep & (artist of t) & sep & (album of t) & sep & ((duration of t) as text) & sep & ((player position) as text) & sep & ((player state) as text)
  end tell
else
  return ""
end if"#;

pub struct MacSpotifyPlayer;

impl Player for MacSpotifyPlayer {
    fn now_playing(&self) -> Result<Option<NowPlaying>, PlayerError> {
        let out = Command::new("osascript")
            .arg("-e")
            .arg(SCRIPT)
            .output()
            .map_err(|e| PlayerError(format!("osascript: {e}")))?;
        if !out.status.success() {
            return Err(PlayerError(String::from_utf8_lossy(&out.stderr).trim().to_string()));
        }
        parse_output(&String::from_utf8_lossy(&out.stdout))
    }
}

/// AppleScript usa o separador decimal do sistema (vírgula no pt-BR).
fn parse_num(s: &str) -> Result<f64, PlayerError> {
    s.trim()
        .replace(',', ".")
        .parse::<f64>()
        .map_err(|_| PlayerError(format!("número inválido do osascript: {s:?}")))
}

pub fn parse_output(s: &str) -> Result<Option<NowPlaying>, PlayerError> {
    let s = s.trim_end_matches(['\n', '\r']);
    if s.trim().is_empty() {
        return Ok(None);
    }
    let parts: Vec<&str> = s.split('\u{1f}').collect();
    if parts.len() != 6 {
        return Err(PlayerError(format!("saída inesperada do osascript: {} campos", parts.len())));
    }
    Ok(Some(NowPlaying {
        title: parts[0].to_string(),
        artist: parts[1].to_string(),
        album: parts[2].to_string(),
        duration_ms: parse_num(parts[3])?.round().max(0.0) as u64,
        position_ms: (parse_num(parts[4])? * 1000.0).round().max(0.0) as u64,
        is_playing: parts[5].trim() == "playing",
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn out(fields: [&str; 6]) -> String {
        format!("{}\n", fields.join("\u{1f}"))
    }

    #[test]
    fn parses_playing_track() {
        let np = parse_output(&out(["Canção Teste", "Banda Fictícia", "Álbum Inventado", "180400", "12.5", "playing"]))
            .unwrap()
            .unwrap();
        assert_eq!(np.title, "Canção Teste");
        assert_eq!(np.artist, "Banda Fictícia");
        assert_eq!(np.album, "Álbum Inventado");
        assert_eq!(np.duration_ms, 180_400);
        assert_eq!(np.position_ms, 12_500);
        assert!(np.is_playing);
    }

    #[test]
    fn parses_paused() {
        let np = parse_output(&out(["a", "b", "c", "1000", "0", "paused"])).unwrap().unwrap();
        assert!(!np.is_playing);
    }

    #[test]
    fn empty_output_is_none() {
        assert_eq!(parse_output("").unwrap(), None);
        assert_eq!(parse_output("\n").unwrap(), None);
    }

    #[test]
    fn decimal_comma_locale() {
        let np = parse_output(&out(["a", "b", "c", "215000", "12,345", "playing"])).unwrap().unwrap();
        assert_eq!(np.position_ms, 12_345);
        let np = parse_output(&out(["a", "b", "c", "2,15E+5", "1,5", "playing"])).unwrap().unwrap();
        assert_eq!(np.duration_ms, 215_000);
        assert_eq!(np.position_ms, 1_500);
    }

    #[test]
    fn title_with_pipe_is_kept() {
        let np = parse_output(&out(["Parte 1 | Parte 2", "b", "c", "1000", "0", "playing"])).unwrap().unwrap();
        assert_eq!(np.title, "Parte 1 | Parte 2");
    }

    #[test]
    fn wrong_field_count_is_error() {
        assert!(parse_output("só um campo").is_err());
        assert!(parse_output(&out(["a", "b", "c", "x", "0", "playing"])).is_err());
    }
}
