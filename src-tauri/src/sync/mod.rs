use crate::lyrics::lrc::Lyrics;
use crate::player::{NowPlaying, TrackKey};
use std::collections::HashMap;
use std::time::Instant;

pub const SEEK_THRESHOLD_MS: i64 = 1500;
pub const OFFSET_STEP_MS: i64 = 250;

#[derive(Debug, Clone, PartialEq)]
pub enum Effect {
    Hide,
    Show,
    LineChanged(i64),
    LyricsLoaded(TrackKey, Vec<String>),
    FetchLyrics(NowPlaying),
    TrackChanged(Option<String>),
}

struct Anchor {
    position_ms: u64,
    at: Instant,
    playing: bool,
}

pub struct Engine {
    track: Option<TrackKey>,
    lyrics: Option<Lyrics>,
    anchor: Option<Anchor>,
    last_index: Option<i64>,
    showing: bool,
    offsets: HashMap<String, i64>,
}

/// Anúncios e podcasts não têm letra: duração 0 ou título/artista vazio.
fn is_music(np: &NowPlaying) -> bool {
    np.duration_ms > 0 && !np.title.trim().is_empty() && !np.artist.trim().is_empty()
}

impl Engine {
    pub fn new(offsets: HashMap<String, i64>) -> Self {
        Self { track: None, lyrics: None, anchor: None, last_index: None, showing: false, offsets }
    }

    pub fn offsets(&self) -> &HashMap<String, i64> {
        &self.offsets
    }

    pub fn current_track(&self) -> Option<&TrackKey> {
        self.track.as_ref()
    }

    pub fn current_lines(&self) -> Option<Vec<String>> {
        self.lyrics.as_ref().map(|l| l.texts())
    }

    fn hide(&mut self, fx: &mut Vec<Effect>) {
        if self.showing {
            self.showing = false;
            fx.push(Effect::Hide);
        }
        self.last_index = None;
    }

    fn anchor_to(&mut self, np: &NowPlaying, now: Instant) {
        self.anchor = Some(Anchor { position_ms: np.position_ms, at: now, playing: np.is_playing });
    }

    pub fn on_poll(&mut self, np: Option<NowPlaying>, now: Instant) -> Vec<Effect> {
        let mut fx = Vec::new();
        let Some(np) = np.filter(is_music) else {
            if self.track.take().is_some() {
                self.lyrics = None;
                self.anchor = None;
                fx.push(Effect::TrackChanged(None));
            }
            self.hide(&mut fx);
            return fx;
        };

        let key = np.key();
        if self.track.as_ref() != Some(&key) {
            self.hide(&mut fx);
            self.track = Some(key);
            self.lyrics = None;
            self.anchor_to(&np, now);
            fx.push(Effect::TrackChanged(Some(np.display_title())));
            fx.push(Effect::FetchLyrics(np));
            return fx;
        }

        let est = self.estimate(now).unwrap_or(np.position_ms);
        let drift = np.position_ms as i64 - est as i64;
        let playing_changed = self.anchor.as_ref().is_none_or(|a| a.playing != np.is_playing);
        if playing_changed || drift.abs() > SEEK_THRESHOLD_MS {
            self.anchor_to(&np, now);
        }
        if !np.is_playing {
            self.hide(&mut fx);
        }
        fx
    }

    pub fn estimate(&self, now: Instant) -> Option<u64> {
        let a = self.anchor.as_ref()?;
        Some(if a.playing {
            a.position_ms + now.saturating_duration_since(a.at).as_millis() as u64
        } else {
            a.position_ms
        })
    }

    pub fn on_lyrics(&mut self, key: &TrackKey, lyrics: Option<Lyrics>) -> Vec<Effect> {
        if self.track.as_ref() != Some(key) {
            return Vec::new();
        }
        match lyrics {
            Some(l) if !l.lines.is_empty() => {
                let texts = l.texts();
                self.lyrics = Some(l);
                self.last_index = None;
                vec![Effect::LyricsLoaded(key.clone(), texts)]
            }
            _ => {
                self.lyrics = None;
                let mut fx = Vec::new();
                self.hide(&mut fx);
                fx
            }
        }
    }

    fn current_index(&self, now: Instant) -> Option<i64> {
        let lyrics = self.lyrics.as_ref()?;
        let key = self.track.as_ref()?;
        if !self.anchor.as_ref()?.playing {
            return None;
        }
        let offset = self.offsets.get(&key.id()).copied().unwrap_or(0);
        let pos = (self.estimate(now)? as i64 + offset).max(0) as u64;
        Some(lyrics.line_at(pos).map_or(-1, |i| i as i64))
    }

    pub fn on_tick(&mut self, now: Instant) -> Vec<Effect> {
        let Some(idx) = self.current_index(now) else { return Vec::new() };
        let mut fx = Vec::new();
        if !self.showing {
            self.showing = true;
            fx.push(Effect::Show);
        }
        if self.last_index != Some(idx) {
            self.last_index = Some(idx);
            fx.push(Effect::LineChanged(idx));
        }
        fx
    }

    pub fn adjust_offset(&mut self, delta: i64) -> Option<i64> {
        let id = self.track.as_ref()?.id();
        let v = self.offsets.entry(id.clone()).or_insert(0);
        *v += delta;
        let now = *v;
        if now == 0 {
            self.offsets.remove(&id);
        }
        Some(now)
    }

    pub fn reset_offset(&mut self) -> Option<i64> {
        let id = self.track.as_ref()?.id();
        self.offsets.remove(&id);
        Some(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lyrics::lrc::parse_lrc;
    use std::time::Duration;

    fn np(title: &str, pos: u64, playing: bool) -> NowPlaying {
        NowPlaying {
            title: title.into(),
            artist: "Banda Fictícia".into(),
            album: "Álbum Inventado".into(),
            duration_ms: 180_000,
            position_ms: pos,
            is_playing: playing,
        }
    }

    fn lyrics() -> Lyrics {
        parse_lrc("[00:01.00]um\n[00:05.00]dois\n[00:09.00]\n[00:12.00]três")
    }

    fn ms(n: u64) -> Duration {
        Duration::from_millis(n)
    }

    /// Engine já com a faixa "A" tocando a partir de `pos` e letra carregada.
    fn loaded(pos: u64, t0: Instant) -> Engine {
        let mut e = Engine::new(HashMap::new());
        e.on_poll(Some(np("A", pos, true)), t0);
        e.on_lyrics(&np("A", 0, true).key(), Some(lyrics()));
        e
    }

    #[test]
    fn new_track_requests_lyrics() {
        let mut e = Engine::new(HashMap::new());
        let fx = e.on_poll(Some(np("A", 0, true)), Instant::now());
        assert_eq!(
            fx,
            vec![Effect::TrackChanged(Some("A — Banda Fictícia".into())), Effect::FetchLyrics(np("A", 0, true))]
        );
    }

    #[test]
    fn lyrics_loaded_then_tick_shows_current_line() {
        let t0 = Instant::now();
        let mut e = Engine::new(HashMap::new());
        e.on_poll(Some(np("A", 6000, true)), t0);
        let key = np("A", 0, true).key();
        assert_eq!(
            e.on_lyrics(&key, Some(lyrics())),
            vec![Effect::LyricsLoaded(key.clone(), vec!["um".into(), "dois".into(), "".into(), "três".into()])]
        );
        assert_eq!(e.on_tick(t0), vec![Effect::Show, Effect::LineChanged(1)]);
        assert_eq!(e.on_tick(t0 + ms(100)), vec![]);
    }

    #[test]
    fn estimates_between_polls() {
        let t0 = Instant::now();
        let mut e = loaded(4000, t0);
        assert_eq!(e.on_tick(t0), vec![Effect::Show, Effect::LineChanged(0)]);
        assert_eq!(e.estimate(t0 + ms(1500)), Some(5500));
        assert_eq!(e.on_tick(t0 + ms(1500)), vec![Effect::LineChanged(1)]);
    }

    #[test]
    fn before_first_line_is_minus_one() {
        let t0 = Instant::now();
        let mut e = loaded(200, t0);
        assert_eq!(e.on_tick(t0), vec![Effect::Show, Effect::LineChanged(-1)]);
    }

    #[test]
    fn pause_hides_and_resume_shows() {
        let t0 = Instant::now();
        let mut e = loaded(6000, t0);
        e.on_tick(t0);
        assert_eq!(e.on_poll(Some(np("A", 6000, false)), t0 + ms(1000)), vec![Effect::Hide]);
        assert_eq!(e.on_tick(t0 + ms(1100)), vec![]);
        assert_eq!(e.estimate(t0 + ms(5000)), Some(6000));
        e.on_poll(Some(np("A", 6000, true)), t0 + ms(2000));
        assert_eq!(e.on_tick(t0 + ms(2000)), vec![Effect::Show, Effect::LineChanged(1)]);
    }

    #[test]
    fn seek_resyncs() {
        let t0 = Instant::now();
        let mut e = loaded(2000, t0);
        e.on_tick(t0);
        e.on_poll(Some(np("A", 13_000, true)), t0 + ms(1000));
        assert_eq!(e.estimate(t0 + ms(1000)), Some(13_000));
        assert_eq!(e.on_tick(t0 + ms(1000)), vec![Effect::LineChanged(3)]);
    }

    #[test]
    fn small_drift_keeps_estimate() {
        let t0 = Instant::now();
        let mut e = loaded(2000, t0);
        e.on_poll(Some(np("A", 3800, true)), t0 + ms(1000)); // estimado 3000, desvio 800
        assert_eq!(e.estimate(t0 + ms(1000)), Some(3000));
    }

    #[test]
    fn replay_of_same_track_resyncs_without_refetch() {
        let t0 = Instant::now();
        let mut e = loaded(179_000, t0);
        e.on_tick(t0);
        let fx = e.on_poll(Some(np("A", 500, true)), t0 + ms(1000));
        assert!(!fx.iter().any(|f| matches!(f, Effect::FetchLyrics(_))));
        assert_eq!(e.on_tick(t0 + ms(1000)), vec![Effect::LineChanged(-1)]);
    }

    #[test]
    fn track_change_hides_and_ignores_stale_lyrics() {
        let t0 = Instant::now();
        let mut e = loaded(6000, t0);
        e.on_tick(t0);
        let fx = e.on_poll(Some(np("B", 0, true)), t0 + ms(1000));
        assert_eq!(
            fx,
            vec![
                Effect::Hide,
                Effect::TrackChanged(Some("B — Banda Fictícia".into())),
                Effect::FetchLyrics(np("B", 0, true)),
            ]
        );
        // letra da faixa A chega atrasada: ignorada
        assert_eq!(e.on_lyrics(&np("A", 0, true).key(), Some(lyrics())), vec![]);
        assert_eq!(e.on_tick(t0 + ms(1100)), vec![]);
    }

    #[test]
    fn nothing_playing_clears_track() {
        let t0 = Instant::now();
        let mut e = loaded(6000, t0);
        e.on_tick(t0);
        assert_eq!(e.on_poll(None, t0 + ms(1000)), vec![Effect::TrackChanged(None), Effect::Hide]);
        assert_eq!(e.current_track(), None);
    }

    #[test]
    fn ads_and_podcasts_are_treated_as_nothing() {
        let mut e = Engine::new(HashMap::new());
        let mut ad = np("Anúncio", 0, true);
        ad.duration_ms = 0;
        assert_eq!(e.on_poll(Some(ad), Instant::now()), vec![]);
        let mut no_artist = np("Episódio", 0, true);
        no_artist.artist = "  ".into();
        assert_eq!(e.on_poll(Some(no_artist), Instant::now()), vec![]);
        assert_eq!(e.current_track(), None);
    }

    #[test]
    fn missing_lyrics_keeps_hidden() {
        let t0 = Instant::now();
        let mut e = Engine::new(HashMap::new());
        e.on_poll(Some(np("A", 6000, true)), t0);
        assert_eq!(e.on_lyrics(&np("A", 0, true).key(), None), vec![]);
        assert_eq!(e.on_tick(t0), vec![]);
    }

    #[test]
    fn offset_shifts_line_and_clamps_at_zero() {
        let t0 = Instant::now();
        let mut e = loaded(4800, t0);
        assert_eq!(e.on_tick(t0), vec![Effect::Show, Effect::LineChanged(0)]);
        assert_eq!(e.adjust_offset(OFFSET_STEP_MS), Some(250));
        assert_eq!(e.on_tick(t0), vec![Effect::LineChanged(1)]);
        assert_eq!(e.offsets().get(&np("A", 0, true).key().id()), Some(&250));
        assert_eq!(e.reset_offset(), Some(0));
        assert!(e.offsets().is_empty());

        let mut e = loaded(100, t0);
        e.adjust_offset(-500);
        assert_eq!(e.on_tick(t0), vec![Effect::Show, Effect::LineChanged(-1)]);
    }

    #[test]
    fn offset_without_track_is_none() {
        let mut e = Engine::new(HashMap::new());
        assert_eq!(e.adjust_offset(250), None);
    }
}
