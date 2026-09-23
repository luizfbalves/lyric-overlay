pub mod config;
pub mod lyrics;
pub mod player;
pub mod sync;
pub mod translate;

pub fn run() {
    tauri::Builder::default()
        .run(tauri::generate_context!())
        .expect("erro ao iniciar o Lyric Overlay");
}
