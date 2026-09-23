pub mod commands;
pub mod config;
pub mod geometry;
pub mod icon;
pub mod links;
pub mod lyrics;
pub mod overlay;
pub mod player;
pub mod prefs;
pub mod shortcuts;
pub mod sink;
pub mod state;
pub mod sync;
pub mod translate;
pub mod tray;

use std::sync::Arc;
use std::time::Duration;
use tauri::{Emitter, Manager};

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(shortcuts::plugin())
        .invoke_handler(tauri::generate_handler![
            commands::get_overlay_init,
            commands::finish_edit,
            commands::get_settings,
            commands::set_appearance,
            commands::set_translation,
            commands::open_link,
        ])
        .setup(|app| {
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);

            let handle = app.handle().clone();
            let config_path = app.path().app_config_dir()?.join("config.json");
            let cache_dir = app.path().app_cache_dir()?.join("translations");
            config::migrate_legacy(&config_path);
            let mut cfg = config::load(&config_path);
            if !translate::ENABLED {
                cfg.translation.mode = config::Mode::Original;
            }

            let status_handle = handle.clone();
            let translation = Arc::new(translate::service::TranslationService::new(
                cache_dir,
                translate::service::TranslateSettings {
                    mode: cfg.translation.mode,
                    target: cfg.translation.target_lang,
                },
                translate::PROXY_URL,
                Duration::from_secs(10),
                Box::new(move |s| {
                    let _ = status_handle.emit("translate-status", s);
                }),
            ));

            let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
            let engine = sync::Engine::new(cfg.offsets.clone());
            app.manage(state::AppState::new(cfg, config_path, translation.clone(), tx));

            tray::build(&handle)?;
            overlay::setup(&handle)?;
            shortcuts::register(&handle);

            let deps = sync::runtime::Deps {
                player: player::system_player(),
                lyrics: Arc::new(lyrics::CachedLyrics::new(lyrics::LrclibClient::new(lyrics::LRCLIB_URL))),
                translator: translation,
                sink: Arc::new(sink::TauriSink::new(handle.clone())),
            };
            tauri::async_runtime::spawn(sync::runtime::run(deps, engine, rx, Default::default()));
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("erro ao iniciar o Verso");
}
