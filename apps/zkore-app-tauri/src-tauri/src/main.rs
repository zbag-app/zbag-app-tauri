// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    let state = zkore_app_tauri_lib::state::AppState::new()
        .expect("failed to initialize application state");

    tauri::Builder::default()
        .manage(state)
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_shell::init())
        .invoke_handler(tauri::generate_handler![
            // Wallet
            zkore_app_tauri_lib::commands::wallet::zkore_create_wallet,
            zkore_app_tauri_lib::commands::wallet::zkore_load_wallet,
            zkore_app_tauri_lib::commands::wallet::zkore_list_wallets,
            zkore_app_tauri_lib::commands::wallet::zkore_get_wallet_status,
            zkore_app_tauri_lib::commands::wallet::zkore_unlock_wallet,
            zkore_app_tauri_lib::commands::wallet::zkore_lock_wallet,
            zkore_app_tauri_lib::commands::wallet::zkore_reauth_wallet,
            zkore_app_tauri_lib::commands::wallet::zkore_view_seed_phrase,
            // Address
            zkore_app_tauri_lib::commands::address::zkore_get_receive_address,
            // Sync
            zkore_app_tauri_lib::commands::sync::zkore_start_sync,
            zkore_app_tauri_lib::commands::sync::zkore_stop_sync,
            zkore_app_tauri_lib::commands::sync::zkore_get_sync_progress,
            // Balance
            zkore_app_tauri_lib::commands::balance::zkore_get_balance,
            // Transactions
            zkore_app_tauri_lib::commands::transaction::zkore_list_transactions,
            zkore_app_tauri_lib::commands::transaction::zkore_prepare_send,
            zkore_app_tauri_lib::commands::transaction::zkore_confirm_send,
            zkore_app_tauri_lib::commands::transaction::zkore_cancel_send,
            zkore_app_tauri_lib::commands::transaction::zkore_retry_broadcast,
            zkore_app_tauri_lib::commands::transaction::zkore_shield_funds,
            // Backup
            zkore_app_tauri_lib::commands::backup::zkore_get_backup_challenge,
            zkore_app_tauri_lib::commands::backup::zkore_verify_backup,
            zkore_app_tauri_lib::commands::backup::zkore_restore_wallet,
            // Keystone
            zkore_app_tauri_lib::commands::keystone::zkore_import_ufvk,
            zkore_app_tauri_lib::commands::keystone::zkore_build_signing_request,
            zkore_app_tauri_lib::commands::keystone::zkore_finalize_signing,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
