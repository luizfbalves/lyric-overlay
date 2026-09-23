use crate::config::WindowPos;
use crate::geometry::{self, Rect};
use crate::state::AppState;
use serde_json::json;
use std::sync::atomic::Ordering;
use tauri::{AppHandle, Emitter, Manager, Monitor, PhysicalPosition, PhysicalSize, WebviewWindow};

pub const LABEL: &str = "overlay";

fn window(app: &AppHandle) -> Option<WebviewWindow> {
    app.get_webview_window(LABEL)
}

fn rect(m: &Monitor) -> Rect {
    Rect { x: m.position().x, y: m.position().y, w: m.size().width, h: m.size().height }
}

pub fn setup(app: &AppHandle) -> tauri::Result<()> {
    let Some(win) = window(app) else { return Ok(()) };
    win.set_ignore_cursor_events(true)?;
    geometry_inner(app, &win, true)?;
    win.show()?;
    Ok(())
}

pub fn apply_geometry(app: &AppHandle) {
    if let Some(win) = window(app) {
        if let Err(e) = geometry_inner(app, &win, false) {
            eprintln!("geometria do overlay: {e}");
        }
    }
}

fn geometry_inner(app: &AppHandle, win: &WebviewWindow, initial: bool) -> tauri::Result<()> {
    let st = app.state::<AppState>();
    let cfg = st.config();
    let monitors: Vec<Rect> = win.available_monitors()?.iter().map(rect).collect();
    let target = if initial { win.primary_monitor()? } else { win.current_monitor()? };
    let Some(mon) = target.or(win.primary_monitor()?) else { return Ok(()) };
    let sf = mon.scale_factor();
    let size = geometry::overlay_size(cfg.appearance.size, cfg.translation.mode, rect(&mon), sf);

    let pos = if initial {
        match cfg.window {
            Some(p) if geometry::center_on_any((p.x, p.y), size, &monitors) => (p.x, p.y),
            _ => geometry::default_position(rect(&mon), size, sf),
        }
    } else {
        let cur = win.outer_position()?;
        let old = win.outer_size()?;
        geometry::keep_center((cur.x, cur.y), (old.width, old.height), size)
    };

    win.set_size(PhysicalSize::new(size.0, size.1))?;
    win.set_position(PhysicalPosition::new(pos.0, pos.1))?;
    if !initial {
        st.update_config(|c| c.window = Some(WindowPos { x: pos.0, y: pos.1 }));
    }
    Ok(())
}

fn save_position(app: &AppHandle, win: &WebviewWindow) {
    if let Ok(p) = win.outer_position() {
        app.state::<AppState>().update_config(|c| c.window = Some(WindowPos { x: p.x, y: p.y }));
    }
}

pub fn toggle_edit(app: &AppHandle) {
    let Some(win) = window(app) else { return };
    let st = app.state::<AppState>();
    let on = !st.edit_mode.load(Ordering::SeqCst);
    st.edit_mode.store(on, Ordering::SeqCst);
    if let Err(e) = win.set_ignore_cursor_events(!on) {
        eprintln!("ignore_cursor_events: {e}");
    }
    if on {
        let _ = win.show();
    } else {
        save_position(app, &win);
    }
    let _ = app.emit("edit-mode", json!({ "on": on }));
}

/// Se o overlay estiver em modo de edição (ex.: ao sair pelo menu "Sair"), salva a posição
/// atual da janela na config antes de encerrar — mesmo caminho usado por `toggle_edit` ao
/// desligar o modo de edição.
pub fn save_position_if_editing(app: &AppHandle) {
    let st = app.state::<AppState>();
    if st.edit_mode.load(Ordering::SeqCst) {
        if let Some(win) = window(app) {
            save_position(app, &win);
        }
    }
}

pub fn toggle_visible(app: &AppHandle) {
    let Some(win) = window(app) else { return };
    let r = if win.is_visible().unwrap_or(true) { win.hide() } else { win.show() };
    if let Err(e) = r {
        eprintln!("mostrar/ocultar: {e}");
    }
}
