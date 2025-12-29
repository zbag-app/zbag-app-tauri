pub mod commands;
pub mod events;
pub mod state;
pub mod windows;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let state = state::AppState::new().expect("failed to initialize application state");

    tauri::Builder::default()
        .manage(state)
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_shell::init())
        .invoke_handler(tauri::generate_handler![
            // Wallet
            commands::wallet::zkore_create_wallet,
            commands::wallet::zkore_load_wallet,
            commands::wallet::zkore_list_wallets,
            commands::wallet::zkore_get_wallet_status,
            commands::wallet::zkore_unlock_wallet,
            commands::wallet::zkore_lock_wallet,
            commands::wallet::zkore_reauth_wallet,
            commands::wallet::zkore_view_seed_phrase,
            // Address
            commands::address::zkore_get_receive_address,
            // Sync
            commands::sync::zkore_start_sync,
            commands::sync::zkore_stop_sync,
            commands::sync::zkore_get_sync_progress,
            // Balance
            commands::balance::zkore_get_balance,
            // Backup
            commands::backup::zkore_get_backup_challenge,
            commands::backup::zkore_verify_backup,
            // Keystone
            commands::keystone::zkore_import_ufvk,
            // Swaps
            commands::swap::zkore_request_swap_quote,
            commands::swap::zkore_start_swap,
            commands::swap::zkore_get_swap_status,
            commands::swap::zkore_list_swaps,
            // Logs
            commands::logs::zkore_get_log_location,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
