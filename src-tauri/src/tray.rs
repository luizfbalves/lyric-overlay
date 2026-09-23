use crate::config::Mode;
use tauri::AppHandle;

pub struct TrayHandles;

pub fn build(_app: &AppHandle) -> tauri::Result<()> {
    Ok(())
}

pub fn set_track_title(_app: &AppHandle, _title: Option<String>) {}

pub fn set_mode_checks(_app: &AppHandle, _mode: Mode) {}
