use crate::config::{Mode, TranslationCfg};
use crate::icon::{note_rgba, ICON_PX};
use crate::links::{self, Link};
use crate::state::AppState;
use crate::sync::runtime::SyncCmd;
use crate::{commands, overlay, prefs};
use tauri::image::Image;
use tauri::menu::{CheckMenuItem, IconMenuItem, IsMenuItem, Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{AppHandle, Manager, Wry};

const BMC_PNG: &[u8] = include_bytes!("../icons/bmc-menu.png");
const NOTHING_PLAYING: &str = "Nada tocando";

pub struct TrayHandles {
    pub title: MenuItem<Wry>,
    pub modes: Vec<(Mode, CheckMenuItem<Wry>)>,
}

/// O menu é declarado aqui como lista: para adicionar opções futuras, inclua uma entrada
/// e trate o `id` em `handle`.
enum Entry {
    TrackTitle,
    Separator,
    Header(&'static str),
    Action { id: &'static str, label: &'static str, accel: Option<&'static str> },
    ModeCheck(Mode, &'static str),
    Icon { id: &'static str, label: &'static str, png: &'static [u8] },
}

fn entries() -> Vec<Entry> {
    use Entry::*;
    vec![
        TrackTitle,
        Separator,
        Action { id: "toggle-visible", label: "Mostrar/ocultar letra", accel: None },
        Action { id: "edit", label: "Editar posição", accel: None },
        Action { id: "reset-offset", label: "Resetar offset desta faixa", accel: None },
        Separator,
        Header("Tradução"),
        ModeCheck(Mode::Original, "Só original"),
        ModeCheck(Mode::Translated, "Só tradução"),
        ModeCheck(Mode::Both, "Original + tradução"),
        Separator,
        Action { id: "prefs", label: "Preferências…", accel: Some("CmdOrCtrl+,") },
        Separator,
        Icon { id: "support", label: "Buy me a coffee", png: BMC_PNG },
        Separator,
        Action { id: "quit", label: "Sair", accel: None },
    ]
}

fn mode_id(m: Mode) -> &'static str {
    match m {
        Mode::Original => "mode-original",
        Mode::Translated => "mode-translated",
        Mode::Both => "mode-both",
    }
}

fn mode_from_id(id: &str) -> Option<Mode> {
    [Mode::Original, Mode::Translated, Mode::Both].into_iter().find(|m| mode_id(*m) == id)
}

pub fn build(app: &AppHandle) -> tauri::Result<()> {
    let st = app.state::<AppState>();
    let current = st.config().translation.mode;
    let mut items: Vec<Box<dyn IsMenuItem<Wry>>> = Vec::new();
    let mut title = None;
    let mut modes = Vec::new();

    for e in entries() {
        match e {
            Entry::TrackTitle => {
                let it = MenuItem::with_id(app, "track", NOTHING_PLAYING, false, None::<&str>)?;
                title = Some(it.clone());
                items.push(Box::new(it));
            }
            Entry::Separator => items.push(Box::new(PredefinedMenuItem::separator(app)?)),
            Entry::Header(label) => items.push(Box::new(MenuItem::new(app, label, false, None::<&str>)?)),
            Entry::Action { id, label, accel } => items.push(Box::new(MenuItem::with_id(app, id, label, true, accel)?)),
            Entry::ModeCheck(m, label) => {
                let it = CheckMenuItem::with_id(app, mode_id(m), label, true, m == current, None::<&str>)?;
                modes.push((m, it.clone()));
                items.push(Box::new(it));
            }
            Entry::Icon { id, label, png } => {
                let icon = Image::from_bytes(png)?;
                items.push(Box::new(IconMenuItem::with_id(app, id, label, true, Some(icon), None::<&str>)?));
            }
        }
    }

    let refs: Vec<&dyn IsMenuItem<Wry>> = items.iter().map(|b| b.as_ref()).collect();
    let menu = Menu::with_items(app, &refs)?;

    TrayIconBuilder::with_id("main")
        .icon(Image::new_owned(note_rgba(ICON_PX), ICON_PX, ICON_PX))
        .icon_as_template(true)
        .tooltip("Lyric Overlay")
        .menu(&menu)
        .show_menu_on_left_click(true)
        .on_menu_event(|app, ev| handle(app, ev.id().as_ref()))
        .build(app)?;

    *st.tray.lock().unwrap() = Some(TrayHandles { title: title.expect("item de título"), modes });
    Ok(())
}

fn handle(app: &AppHandle, id: &str) {
    match id {
        "toggle-visible" => overlay::toggle_visible(app),
        "edit" => overlay::toggle_edit(app),
        "reset-offset" => {
            let _ = app.state::<AppState>().cmds.send(SyncCmd::ResetOffset);
        }
        "prefs" => prefs::open(app),
        "support" => {
            if let Err(e) = links::open(app, Link::Support) {
                eprintln!("abrir link: {e}");
            }
        }
        "quit" => {
            overlay::save_position_if_editing(app);
            app.exit(0);
        }
        other => {
            if let Some(mode) = mode_from_id(other) {
                let cur = app.state::<AppState>().config().translation;
                commands::apply_translation(app, TranslationCfg { mode, ..cur });
            }
        }
    }
}

pub fn set_track_title(app: &AppHandle, title: Option<String>) {
    let st = app.state::<AppState>();
    // Clona o handle e solta o lock antes de chamar `set_text`: no Tauri 2.11 essa chamada,
    // fora da main thread, bloqueia esperando a main thread — que pode estar tentando este
    // mesmo lock — causando deadlock se o guard ainda estiver seguro.
    let title_item = st.tray.lock().unwrap().as_ref().map(|h| h.title.clone());
    if let Some(item) = title_item {
        if let Err(e) = item.set_text(title.as_deref().unwrap_or(NOTHING_PLAYING)) {
            eprintln!("atualizar título da bandeja: {e}");
        }
    }
}

pub fn set_mode_checks(app: &AppHandle, mode: Mode) {
    let st = app.state::<AppState>();
    // Mesmo motivo de `set_track_title`: clona os handles e solta o lock antes de chamar
    // `set_checked`, para não travar esperando a main thread que pode querer este lock.
    let items: Vec<(Mode, CheckMenuItem<Wry>)> =
        st.tray.lock().unwrap().as_ref().map(|h| h.modes.clone()).unwrap_or_default();
    for (m, it) in items {
        if let Err(e) = it.set_checked(m == mode) {
            eprintln!("atualizar check de modo da bandeja: {e}");
        }
    }
}
