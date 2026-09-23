use crate::config::{Appearance, Mode, TranslationCfg};
use crate::links::{self, Link};
use crate::state::{AppState, Snapshot};
use crate::sync::runtime::SyncCmd;
use crate::translate::{self, service::TranslateStatus};
use crate::{overlay, tray};
use serde::Serialize;
use serde_json::json;
use std::sync::atomic::Ordering;
use tauri::{AppHandle, Emitter, Manager, State};

#[derive(Serialize)]
pub struct OverlayInit {
    appearance: Appearance,
    mode: Mode,
    snapshot: Snapshot,
    edit: bool,
}

#[derive(Serialize)]
pub struct SettingsView {
    appearance: Appearance,
    translation: TranslationCfg,
    translation_enabled: bool,
    translate_status: TranslateStatus,
}

#[tauri::command]
pub fn get_overlay_init(state: State<'_, AppState>) -> OverlayInit {
    let cfg = state.config();
    OverlayInit {
        appearance: cfg.appearance,
        mode: cfg.translation.mode,
        snapshot: state.snapshot.lock().unwrap().clone(),
        edit: state.edit_mode.load(Ordering::SeqCst),
    }
}

#[tauri::command]
pub fn finish_edit(app: AppHandle, keep: bool) {
    overlay::finish_edit(&app, keep);
}

#[tauri::command]
pub fn get_settings(state: State<'_, AppState>) -> SettingsView {
    let cfg = state.config();
    SettingsView {
        appearance: cfg.appearance,
        translation: cfg.translation,
        translation_enabled: translate::ENABLED,
        translate_status: state.translation.status(),
    }
}

#[tauri::command]
pub fn set_appearance(app: AppHandle, state: State<'_, AppState>, appearance: Appearance) {
    let a = appearance.normalized();
    state.update_config(|c| c.appearance = a.clone());
    overlay::apply_geometry(&app);
    let _ = app.emit("appearance-changed", &a);
}

pub fn apply_translation(app: &AppHandle, t: TranslationCfg) {
    let st = app.state::<AppState>();
    st.update_config(|c| c.translation = t);
    st.translation.set_mode_target(t.mode, t.target_lang);
    overlay::apply_geometry(app);
    tray::set_mode_checks(app, t.mode);
    let _ = app.emit("mode-changed", json!({ "mode": t.mode, "target_lang": t.target_lang }));
    let _ = st.cmds.send(SyncCmd::Retranslate);
}

#[tauri::command]
pub fn set_translation(app: AppHandle, translation: TranslationCfg) {
    apply_translation(&app, translation);
}

#[tauri::command]
pub fn open_link(app: AppHandle, link: Link) -> Result<(), String> {
    links::open(&app, link)
}
