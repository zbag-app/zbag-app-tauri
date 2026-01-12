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
            commands::wallet::zstash_create_wallet,
            commands::wallet::zstash_load_wallet,
            commands::wallet::zstash_list_wallets,
            commands::wallet::zstash_get_wallet_status,
            commands::wallet::zstash_unlock_wallet,
            commands::wallet::zstash_lock_wallet,
            commands::wallet::zstash_reauth_wallet,
            commands::wallet::zstash_view_seed_phrase,
            commands::wallet::zstash_logout_wallet,
            // Address
            commands::address::zstash_get_receive_address,
            // Sync
            commands::sync::zstash_start_sync,
            commands::sync::zstash_stop_sync,
            commands::sync::zstash_get_sync_progress,
            // Balance
            commands::balance::zstash_get_balance,
            // Backup
            commands::backup::zstash_get_backup_challenge,
            commands::backup::zstash_verify_backup,
            // Keystone
            commands::keystone::zstash_import_ufvk,
            commands::keystone::zstash_create_keystone_wallet,
            // Swaps
            commands::swap::zstash_request_swap_quote,
            commands::swap::zstash_start_swap,
            commands::swap::zstash_get_swap_status,
            commands::swap::zstash_list_swaps,
            // Logs
            commands::logs::zstash_get_log_location,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
