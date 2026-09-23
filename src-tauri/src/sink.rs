use crate::state::AppState;
use crate::sync::runtime::{OverlayEvent, Sink};
use crate::tray;
use serde_json::json;
use std::collections::HashMap;
use tauri::{AppHandle, Emitter, Manager};

pub struct TauriSink {
    app: AppHandle,
}

impl TauriSink {
    pub fn new(app: AppHandle) -> Self {
        Self { app }
    }
}

impl Sink for TauriSink {
    fn emit(&self, ev: OverlayEvent) {
        self.app.state::<AppState>().snapshot.lock().unwrap().apply(&ev);
        let r = match &ev {
            OverlayEvent::LyricsLoaded(l) => self.app.emit("lyrics-loaded", json!({ "lines": l })),
            OverlayEvent::TranslationLoaded(l) => self.app.emit("translation-loaded", json!({ "lines": l })),
            OverlayEvent::LineChanged(i) => self.app.emit("line-changed", json!({ "index": i })),
            OverlayEvent::Hide => self.app.emit("hide", ()),
            OverlayEvent::Show => self.app.emit("show", ()),
            OverlayEvent::OffsetChanged(o) => self.app.emit("offset-changed", json!({ "offset_ms": o })),
        };
        if let Err(e) = r {
            eprintln!("emit: {e}");
        }
    }

    fn track_changed(&self, title: Option<String>) {
        tray::set_track_title(&self.app, title);
    }

    fn offsets_changed(&self, offsets: &HashMap<String, i64>) {
        self.app.state::<AppState>().update_config(|c| c.offsets = offsets.clone());
    }
}
