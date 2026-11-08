//! DiskGoblin desktop backend; default builds are independent of GUI libraries.
pub mod commands;
pub mod state;

#[cfg(feature = "tauri-runtime")]
pub fn run() -> tauri::Result<()> {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(std::sync::Arc::new(state::ActiveScanState::default()))
        .invoke_handler(tauri::generate_handler![
            commands::runtime::start_scan,
            commands::runtime::cancel_scan,
            commands::runtime::pick_directory,
            commands::runtime::reveal_path,
        ])
        .run(tauri::generate_context!())
}
