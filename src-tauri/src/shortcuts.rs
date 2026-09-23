use crate::overlay;
use crate::state::AppState;
use crate::sync::runtime::SyncCmd;
use crate::sync::OFFSET_STEP_MS;
use tauri::plugin::TauriPlugin;
use tauri::{AppHandle, Manager, Wry};
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};

fn primary() -> Modifiers {
    if cfg!(target_os = "macos") {
        Modifiers::SUPER
    } else {
        Modifiers::CONTROL
    }
}

fn edit() -> Shortcut {
    Shortcut::new(Some(primary() | Modifiers::SHIFT), Code::KeyL)
}

fn earlier() -> Shortcut {
    Shortcut::new(Some(primary() | Modifiers::SHIFT), Code::ArrowLeft)
}

fn later() -> Shortcut {
    Shortcut::new(Some(primary() | Modifiers::SHIFT), Code::ArrowRight)
}

fn send(app: &AppHandle, cmd: SyncCmd) {
    let _ = app.state::<AppState>().cmds.send(cmd);
}

pub fn plugin() -> TauriPlugin<Wry> {
    tauri_plugin_global_shortcut::Builder::new()
        .with_handler(|app, sc, ev| {
            if ev.state() != ShortcutState::Pressed {
                return;
            }
            if *sc == edit() {
                overlay::toggle_edit(app);
            } else if *sc == earlier() {
                send(app, SyncCmd::AdjustOffset(-OFFSET_STEP_MS));
            } else if *sc == later() {
                send(app, SyncCmd::AdjustOffset(OFFSET_STEP_MS));
            }
        })
        .build()
}

pub fn register(app: &AppHandle) {
    let gs = app.global_shortcut();
    for sc in [edit(), earlier(), later()] {
        if let Err(e) = gs.register(sc) {
            eprintln!("atalho {sc:?} indisponível: {e}");
        }
    }
}
