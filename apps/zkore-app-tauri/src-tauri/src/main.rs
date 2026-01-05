// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::sync::Arc;
use tauri::Manager;

fn main() {
    let state = zkore_app_tauri_lib::state::AppState::new()
        .expect("failed to initialize application state");

    tauri::Builder::default()
        .manage(state)
        .setup(|app| {
            let state = app.state::<zkore_app_tauri_lib::state::AppState>();

            let _ = state.tor_manager.start_if_enabled();

            let app_handle = app.handle().clone();

            {
                let wallet_manager = Arc::clone(&state.wallet_manager);
                let app_handle = app_handle.clone();
                if let Ok(mut mgr) = wallet_manager.lock() {
                    mgr.set_wallet_status_handler(Arc::new(move |event| {
                        let _ = zkore_app_tauri_lib::events::emit_wallet_status(&app_handle, event);
                    }));
                }
            }

            let mut rx = state.tor_manager.subscribe();

            tauri::async_runtime::spawn(async move {
                let _ =
                    zkore_app_tauri_lib::events::emit_tor_status(&app_handle, rx.borrow().clone());
                while rx.changed().await.is_ok() {
                    let _ = zkore_app_tauri_lib::events::emit_tor_status(
                        &app_handle,
                        rx.borrow().clone(),
                    );
                }
            });

            Ok(())
        })
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
            zkore_app_tauri_lib::commands::wallet::zkore_logout_wallet,
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
            // Swaps
            zkore_app_tauri_lib::commands::swap::zkore_request_swap_quote,
            zkore_app_tauri_lib::commands::swap::zkore_start_swap,
            zkore_app_tauri_lib::commands::swap::zkore_get_swap_status,
            zkore_app_tauri_lib::commands::swap::zkore_list_swaps,
            // Tor
            zkore_app_tauri_lib::commands::tor::zkore_set_tor_enabled,
            zkore_app_tauri_lib::commands::tor::zkore_get_tor_state,
            // Logs
            zkore_app_tauri_lib::commands::logs::zkore_get_log_location,
            // Servers
            zkore_app_tauri_lib::commands::server::zkore_add_server,
            zkore_app_tauri_lib::commands::server::zkore_set_default_server,
            zkore_app_tauri_lib::commands::server::zkore_test_server,
            zkore_app_tauri_lib::commands::server::zkore_list_servers,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
