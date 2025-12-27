// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    let state = zkore_app_tauri_lib::state::AppState::new()
        .expect("failed to initialize application state");

    tauri::Builder::default()
        .manage(state)
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_shell::init())
        .invoke_handler(tauri::generate_handler![zkore_app_tauri_lib::commands::greet])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
