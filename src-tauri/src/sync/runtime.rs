use super::{Effect, Engine};
use crate::lyrics::lrc::Lyrics;
use crate::lyrics::LyricsSource;
use crate::player::{NowPlaying, Player, TrackKey};
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};
use tokio::time::MissedTickBehavior;

#[derive(Debug, Clone, PartialEq)]
pub enum OverlayEvent {
    LyricsLoaded(Vec<String>),
    TranslationLoaded(Vec<String>),
    LineChanged(i64),
    Hide,
    Show,
    OffsetChanged(i64),
}

pub trait Sink: Send + Sync {
    fn emit(&self, ev: OverlayEvent);
    fn track_changed(&self, title: Option<String>);
    fn offsets_changed(&self, offsets: &HashMap<String, i64>);
}

#[async_trait]
pub trait TrackTranslator: Send + Sync {
    async fn translate_track(&self, key: &TrackKey, lines: &[String]) -> Option<Vec<String>>;
    /// Identificador do idioma alvo atual (ex.: código DeepL "PT-BR"). Usado para descartar
    /// traduções que chegam depois de uma troca de idioma-alvo.
    fn current_target(&self) -> String;
}

#[derive(Debug)]
pub enum SyncCmd {
    AdjustOffset(i64),
    ResetOffset,
    Retranslate,
}

pub struct Timing {
    pub poll: Duration,
    pub tick: Duration,
    /// Esperas entre novas tentativas quando a busca de letra falha por rede (ex.: LRCLIB
    /// sobrecarregado respondendo 503). Sem isso a faixa ficaria sem letra até trocar.
    pub lyrics_retry: Vec<Duration>,
}

impl Default for Timing {
    fn default() -> Self {
        Self {
            poll: Duration::from_secs(1),
            tick: Duration::from_millis(100),
            lyrics_retry: [2, 5, 10, 20, 30, 60, 60].map(Duration::from_secs).to_vec(),
        }
    }
}

pub struct Deps {
    pub player: Arc<dyn Player>,
    pub lyrics: Arc<dyn LyricsSource>,
    pub translator: Arc<dyn TrackTranslator>,
    pub sink: Arc<dyn Sink>,
}

enum Internal {
    Lyrics(TrackKey, Option<Lyrics>),
    // Carrega o idioma-alvo capturado no momento em que a tradução foi pedida, para poder
    // descartar o resultado se o alvo tiver mudado enquanto a tradução estava em voo.
    Translation(TrackKey, String, Option<Vec<String>>),
}

fn spawn_fetch(deps: &Deps, itx: &UnboundedSender<Internal>, retry: &[Duration], np: NowPlaying) {
    let src = deps.lyrics.clone();
    let itx = itx.clone();
    let retry = retry.to_vec();
    tokio::spawn(async move {
        let mut waits = retry.into_iter();
        let res = loop {
            match src.fetch(&np).await {
                Ok(l) => break l,
                Err(e) => {
                    eprintln!("letra: {e}");
                    let Some(w) = waits.next() else { break None };
                    tokio::time::sleep(w).await;
                }
            }
        };
        let _ = itx.send(Internal::Lyrics(np.key(), res));
    });
}

fn spawn_translate(deps: &Deps, itx: &UnboundedSender<Internal>, key: TrackKey, lines: Vec<String>) {
    let tr = deps.translator.clone();
    let itx = itx.clone();
    let target = tr.current_target();
    tokio::spawn(async move {
        let res = tr.translate_track(&key, &lines).await;
        let _ = itx.send(Internal::Translation(key, target, res));
    });
}

fn apply(deps: &Deps, itx: &UnboundedSender<Internal>, retry: &[Duration], fx: Vec<Effect>) {
    for e in fx {
        match e {
            Effect::Hide => deps.sink.emit(OverlayEvent::Hide),
            Effect::Show => deps.sink.emit(OverlayEvent::Show),
            Effect::LineChanged(i) => deps.sink.emit(OverlayEvent::LineChanged(i)),
            Effect::TrackChanged(t) => deps.sink.track_changed(t),
            Effect::FetchLyrics(np) => spawn_fetch(deps, itx, retry, np),
            Effect::LyricsLoaded(key, lines) => {
                deps.sink.emit(OverlayEvent::LyricsLoaded(lines.clone()));
                spawn_translate(deps, itx, key, lines);
            }
        }
    }
}

pub async fn run(deps: Deps, mut engine: Engine, mut cmds: UnboundedReceiver<SyncCmd>, timing: Timing) {
    let (itx, mut irx) = mpsc::unbounded_channel::<Internal>();
    let mut poll = tokio::time::interval(timing.poll);
    poll.set_missed_tick_behavior(MissedTickBehavior::Skip);
    let mut tick = tokio::time::interval(timing.tick);
    tick.set_missed_tick_behavior(MissedTickBehavior::Skip);

    loop {
        tokio::select! {
            _ = poll.tick() => {
                let player = deps.player.clone();
                let np = match tokio::task::spawn_blocking(move || player.now_playing()).await {
                    Ok(Ok(np)) => np,
                    Ok(Err(e)) => { eprintln!("player: {e}"); None }
                    Err(e) => { eprintln!("player (task): {e}"); None }
                };
                let fx = engine.on_poll(np, Instant::now());
                apply(&deps, &itx, &timing.lyrics_retry, fx);
            }
            _ = tick.tick() => {
                let fx = engine.on_tick(Instant::now());
                apply(&deps, &itx, &timing.lyrics_retry, fx);
            }
            Some(msg) = irx.recv() => match msg {
                Internal::Lyrics(key, l) => {
                    let fx = engine.on_lyrics(&key, l);
                    apply(&deps, &itx, &timing.lyrics_retry, fx);
                }
                Internal::Translation(key, target, lines) => {
                    let stale_target = deps.translator.current_target() != target;
                    if engine.current_track() == Some(&key) && !stale_target {
                        deps.sink.emit(OverlayEvent::TranslationLoaded(lines.unwrap_or_default()));
                    }
                }
            },
            cmd = cmds.recv() => match cmd {
                None => break,
                Some(SyncCmd::AdjustOffset(d)) => {
                    if let Some(o) = engine.adjust_offset(d) {
                        deps.sink.emit(OverlayEvent::OffsetChanged(o));
                        deps.sink.offsets_changed(engine.offsets());
                    }
                }
                Some(SyncCmd::ResetOffset) => {
                    if let Some(o) = engine.reset_offset() {
                        deps.sink.emit(OverlayEvent::OffsetChanged(o));
                        deps.sink.offsets_changed(engine.offsets());
                    }
                }
                Some(SyncCmd::Retranslate) => {
                    if let (Some(key), Some(lines)) = (engine.current_track().cloned(), engine.current_lines()) {
                        spawn_translate(&deps, &itx, key, lines);
                    }
                }
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lyrics::lrc::parse_lrc;
    use crate::lyrics::LyricsError;
    use crate::player::PlayerError;
    use std::sync::Mutex;

    struct FakePlayer(Mutex<Option<NowPlaying>>);
    impl Player for FakePlayer {
        fn now_playing(&self) -> Result<Option<NowPlaying>, PlayerError> {
            Ok(self.0.lock().unwrap().clone())
        }
    }

    struct FakeLyrics;
    #[async_trait]
    impl LyricsSource for FakeLyrics {
        async fn fetch(&self, _: &NowPlaying) -> Result<Option<Lyrics>, LyricsError> {
            Ok(Some(parse_lrc("[00:01.00]um\n[00:05.00]dois")))
        }
    }

    /// Falha por rede nas primeiras `fails` chamadas e depois devolve a letra.
    struct FlakyLyrics(Mutex<u32>);
    #[async_trait]
    impl LyricsSource for FlakyLyrics {
        async fn fetch(&self, np: &NowPlaying) -> Result<Option<Lyrics>, LyricsError> {
            let failed = {
                let mut left = self.0.lock().unwrap();
                let f = *left > 0;
                *left = left.saturating_sub(1);
                f
            };
            if failed {
                return Err(LyricsError::Network("HTTP 503 Service Unavailable".into()));
            }
            FakeLyrics.fetch(np).await
        }
    }

    struct FakeTranslator;
    #[async_trait]
    impl TrackTranslator for FakeTranslator {
        async fn translate_track(&self, _: &TrackKey, lines: &[String]) -> Option<Vec<String>> {
            Some(lines.iter().map(|l| format!("tr:{l}")).collect())
        }
        fn current_target(&self) -> String {
            "PT-BR".into()
        }
    }

    /// Tradutor controlável: só resolve `translate_track` quando `release()` é chamado, e
    /// permite trocar o idioma-alvo "atual" enquanto a tradução está em voo — para simular
    /// uma resposta antiga chegando depois de uma troca de idioma.
    struct GatedTranslator {
        target: Mutex<String>,
        gate: tokio::sync::Notify,
    }
    impl GatedTranslator {
        fn new(target: &str) -> Self {
            Self { target: Mutex::new(target.to_string()), gate: tokio::sync::Notify::new() }
        }
        fn set_target(&self, t: &str) {
            *self.target.lock().unwrap() = t.to_string();
        }
        fn release(&self) {
            self.gate.notify_one();
        }
    }
    #[async_trait]
    impl TrackTranslator for GatedTranslator {
        async fn translate_track(&self, _: &TrackKey, lines: &[String]) -> Option<Vec<String>> {
            self.gate.notified().await;
            Some(lines.iter().map(|l| format!("tr:{l}")).collect())
        }
        fn current_target(&self) -> String {
            self.target.lock().unwrap().clone()
        }
    }

    #[derive(Default)]
    struct Recorder {
        events: Mutex<Vec<OverlayEvent>>,
        titles: Mutex<Vec<Option<String>>>,
        offsets: Mutex<Vec<HashMap<String, i64>>>,
    }
    impl Sink for Recorder {
        fn emit(&self, ev: OverlayEvent) {
            self.events.lock().unwrap().push(ev);
        }
        fn track_changed(&self, t: Option<String>) {
            self.titles.lock().unwrap().push(t);
        }
        fn offsets_changed(&self, o: &HashMap<String, i64>) {
            self.offsets.lock().unwrap().push(o.clone());
        }
    }

    #[tokio::test]
    async fn full_cycle_with_fakes() {
        let np = NowPlaying {
            title: "Canção Teste".into(),
            artist: "Banda Fictícia".into(),
            album: "Álbum Inventado".into(),
            duration_ms: 180_000,
            position_ms: 6000,
            is_playing: true,
        };
        let rec = Arc::new(Recorder::default());
        let deps = Deps {
            player: Arc::new(FakePlayer(Mutex::new(Some(np)))),
            lyrics: Arc::new(FakeLyrics),
            translator: Arc::new(FakeTranslator),
            sink: rec.clone(),
        };
        let (tx, rx) = mpsc::unbounded_channel();
        let timing = Timing { poll: Duration::from_millis(20), tick: Duration::from_millis(5), ..Default::default() };
        let handle = tokio::spawn(run(deps, Engine::new(HashMap::new()), rx, timing));

        tokio::time::sleep(Duration::from_millis(200)).await;
        {
            let ev = rec.events.lock().unwrap();
            assert!(ev.contains(&OverlayEvent::LyricsLoaded(vec!["um".into(), "dois".into()])));
            assert!(ev.contains(&OverlayEvent::TranslationLoaded(vec!["tr:um".into(), "tr:dois".into()])));
            assert!(ev.contains(&OverlayEvent::Show));
            assert!(ev.contains(&OverlayEvent::LineChanged(1)));
        }
        assert_eq!(rec.titles.lock().unwrap()[0], Some("Canção Teste — Banda Fictícia".into()));

        tx.send(SyncCmd::AdjustOffset(250)).unwrap();
        tokio::time::sleep(Duration::from_millis(50)).await;
        assert!(rec.events.lock().unwrap().contains(&OverlayEvent::OffsetChanged(250)));
        assert_eq!(rec.offsets.lock().unwrap().last().unwrap().get("Banda Fictícia|Canção Teste|180"), Some(&250));

        drop(tx);
        tokio::time::timeout(Duration::from_secs(1), handle).await.unwrap().unwrap();
    }

    #[tokio::test]
    async fn stale_translation_after_language_switch_is_discarded() {
        let np = NowPlaying {
            title: "Canção Teste".into(),
            artist: "Banda Fictícia".into(),
            album: "Álbum Inventado".into(),
            duration_ms: 180_000,
            position_ms: 6000,
            is_playing: true,
        };
        let rec = Arc::new(Recorder::default());
        let translator = Arc::new(GatedTranslator::new("PT-BR"));
        let deps = Deps {
            player: Arc::new(FakePlayer(Mutex::new(Some(np)))),
            lyrics: Arc::new(FakeLyrics),
            translator: translator.clone(),
            sink: rec.clone(),
        };
        let (_tx, rx) = mpsc::unbounded_channel();
        let timing = Timing { poll: Duration::from_millis(20), tick: Duration::from_millis(5), ..Default::default() };
        let handle = tokio::spawn(run(deps, Engine::new(HashMap::new()), rx, timing));

        // Espera a letra carregar: nesse ponto a tradução já foi disparada (e capturou o
        // alvo "PT-BR"), mas está presa no `gate` do GatedTranslator.
        tokio::time::sleep(Duration::from_millis(100)).await;
        assert!(rec
            .events
            .lock()
            .unwrap()
            .contains(&OverlayEvent::LyricsLoaded(vec!["um".into(), "dois".into()])));
        assert!(!rec.events.lock().unwrap().iter().any(|e| matches!(e, OverlayEvent::TranslationLoaded(_))));

        // Troca de idioma-alvo enquanto a tradução ainda está em voo, depois libera a resposta
        // antiga (ainda no idioma anterior).
        translator.set_target("EN-US");
        translator.release();
        tokio::time::sleep(Duration::from_millis(100)).await;

        assert!(
            !rec.events.lock().unwrap().iter().any(|e| matches!(e, OverlayEvent::TranslationLoaded(_))),
            "tradução presa no idioma antigo não deveria ter sido aplicada"
        );

        handle.abort();
    }

    #[tokio::test]
    async fn lyrics_network_error_is_retried() {
        let np = NowPlaying {
            title: "Canção Teste".into(),
            artist: "Banda Fictícia".into(),
            album: "Álbum Inventado".into(),
            duration_ms: 180_000,
            position_ms: 6000,
            is_playing: true,
        };
        let rec = Arc::new(Recorder::default());
        let deps = Deps {
            player: Arc::new(FakePlayer(Mutex::new(Some(np)))),
            lyrics: Arc::new(FlakyLyrics(Mutex::new(2))),
            translator: Arc::new(FakeTranslator),
            sink: rec.clone(),
        };
        let (_tx, rx) = mpsc::unbounded_channel();
        let timing = Timing {
            poll: Duration::from_millis(20),
            tick: Duration::from_millis(5),
            lyrics_retry: vec![Duration::from_millis(10); 3],
        };
        let handle = tokio::spawn(run(deps, Engine::new(HashMap::new()), rx, timing));

        tokio::time::sleep(Duration::from_millis(200)).await;
        {
            let ev = rec.events.lock().unwrap();
            assert!(ev.contains(&OverlayEvent::LyricsLoaded(vec!["um".into(), "dois".into()])));
            assert!(ev.contains(&OverlayEvent::Show));
        }
        handle.abort();
    }
}
