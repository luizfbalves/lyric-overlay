use crate::config::{self, Config};
use crate::sync::runtime::{OverlayEvent, SyncCmd};
use crate::translate::service::TranslationService;
use crate::tray::TrayHandles;
use serde::Serialize;
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc::UnboundedSender;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Snapshot {
    pub lines: Vec<String>,
    pub translation: Option<Vec<String>>,
    pub index: i64,
    pub visible: bool,
}

impl Default for Snapshot {
    fn default() -> Self {
        Self { lines: Vec::new(), translation: None, index: -1, visible: false }
    }
}

impl Snapshot {
    pub fn apply(&mut self, ev: &OverlayEvent) {
        match ev {
            OverlayEvent::LyricsLoaded(l) => {
                self.lines = l.clone();
                self.translation = None;
                self.index = -1;
            }
            OverlayEvent::TranslationLoaded(l) => {
                self.translation = (!l.is_empty()).then(|| l.clone());
            }
            OverlayEvent::LineChanged(i) => self.index = *i,
            OverlayEvent::Hide => self.visible = false,
            OverlayEvent::Show => self.visible = true,
            OverlayEvent::OffsetChanged(_) => {}
        }
    }
}

pub struct AppState {
    pub config: Mutex<Config>,
    pub config_path: PathBuf,
    pub translation: Arc<TranslationService>,
    pub cmds: UnboundedSender<SyncCmd>,
    pub snapshot: Mutex<Snapshot>,
    pub edit_mode: AtomicBool,
    /// Posição da janela quando o modo de edição foi ligado, para "reverter".
    pub edit_origin: Mutex<Option<(i32, i32)>>,
    pub tray: Mutex<Option<TrayHandles>>,
}

impl AppState {
    pub fn new(cfg: Config, config_path: PathBuf, translation: Arc<TranslationService>, cmds: UnboundedSender<SyncCmd>) -> Self {
        Self {
            config: Mutex::new(cfg),
            config_path,
            translation,
            cmds,
            snapshot: Mutex::new(Snapshot::default()),
            edit_mode: AtomicBool::new(false),
            edit_origin: Mutex::new(None),
            tray: Mutex::new(None),
        }
    }

    pub fn config(&self) -> Config {
        self.config.lock().unwrap().clone()
    }

    pub fn update_config(&self, f: impl FnOnce(&mut Config)) {
        // O save acontece com o lock ainda seguro: é I/O pequeno e local, e evita que dois
        // escritores concorrentes disputem o mesmo `config.json.tmp` ou que uma versão velha
        // sobrescreva uma mais nova gravada entre o `clone` e o `save`.
        let mut c = self.config.lock().unwrap();
        f(&mut c);
        if let Err(e) = config::save(&self.config_path, &c) {
            eprintln!("salvar config: {e}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snapshot_follows_events() {
        let mut s = Snapshot::default();
        assert_eq!(s.index, -1);
        s.apply(&OverlayEvent::LyricsLoaded(vec!["um".into(), "dois".into()]));
        s.apply(&OverlayEvent::TranslationLoaded(vec!["tr:um".into(), "tr:dois".into()]));
        s.apply(&OverlayEvent::Show);
        s.apply(&OverlayEvent::LineChanged(1));
        assert_eq!(s.lines.len(), 2);
        assert_eq!(s.translation.as_ref().unwrap()[1], "tr:dois");
        assert!(s.visible);
        assert_eq!(s.index, 1);

        s.apply(&OverlayEvent::TranslationLoaded(vec![]));
        assert_eq!(s.translation, None);
        s.apply(&OverlayEvent::Hide);
        assert!(!s.visible);
        s.apply(&OverlayEvent::LyricsLoaded(vec!["novo".into()]));
        assert_eq!(s.index, -1);
    }
}
