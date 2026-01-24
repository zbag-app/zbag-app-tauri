// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

#[cfg(not(feature = "test-bridge"))]
use std::sync::Arc;
#[cfg(not(feature = "test-bridge"))]
use tauri::Manager;

fn main() {
    // When test-bridge feature is enabled, run only the HTTP bridge server (no Tauri)
    #[cfg(feature = "test-bridge")]
    {
        run_test_bridge_only();
    }

    #[cfg(not(feature = "test-bridge"))]
    run_tauri_app();
}

/// Delegates to the library's test bridge implementation.
#[cfg(feature = "test-bridge")]
fn run_test_bridge_only() {
    zstash_app_tauri_lib::run_test_bridge_only();
}

#[cfg(not(feature = "test-bridge"))]
fn run_tauri_app() {
    let state = zstash_app_tauri_lib::state::AppState::new()
        .expect("failed to initialize application state");

    // Log version at startup
    let version_info = zstash_core::version::VersionInfo::current();
    tracing::info!(
        version = %version_info.version,
        git_commit = version_info.git_commit.as_deref().unwrap_or("release"),
        build_timestamp = %version_info.build_timestamp,
        "zSTASH Desktop starting"
    );

    tauri::Builder::default()
        .manage(state)
        .setup(|app| {
            let state = app.state::<zstash_app_tauri_lib::state::AppState>();

            // Enter tokio runtime context so TorManager can spawn bootstrap task
            let tauri::async_runtime::RuntimeHandle::Tokio(handle) = tauri::async_runtime::handle();
            let _guard = handle.enter();

            if let Err(err) = state.tor_manager.start_if_enabled() {
                tracing::warn!(error = ?err, "failed to start Tor on app launch");
            }

            let app_handle = app.handle().clone();

            {
                let wallet_manager = Arc::clone(&state.wallet_manager);
                let app_handle = app_handle.clone();
                if let Ok(mut mgr) = wallet_manager.lock() {
                    mgr.set_wallet_status_handler(Arc::new(move |event| {
                        let _ =
                            zstash_app_tauri_lib::events::emit_wallet_status(&app_handle, event);
                    }));
                }
            }

            let mut rx = state.tor_manager.subscribe();

            tauri::async_runtime::spawn(async move {
                let _ =
                    zstash_app_tauri_lib::events::emit_tor_status(&app_handle, rx.borrow().clone());
                while rx.changed().await.is_ok() {
                    let _ = zstash_app_tauri_lib::events::emit_tor_status(
                        &app_handle,
                        rx.borrow().clone(),
                    );
                }
            });

            Ok(())
        })
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            // Wallet
            zstash_app_tauri_lib::commands::wallet::zstash_create_wallet,
            zstash_app_tauri_lib::commands::wallet::zstash_load_wallet,
            zstash_app_tauri_lib::commands::wallet::zstash_list_wallets,
            zstash_app_tauri_lib::commands::wallet::zstash_get_wallet_status,
            zstash_app_tauri_lib::commands::wallet::zstash_unlock_wallet,
            zstash_app_tauri_lib::commands::wallet::zstash_lock_wallet,
            zstash_app_tauri_lib::commands::wallet::zstash_reauth_wallet,
            zstash_app_tauri_lib::commands::wallet::zstash_view_seed_phrase,
            zstash_app_tauri_lib::commands::wallet::zstash_logout_wallet,
            // Address
            zstash_app_tauri_lib::commands::address::zstash_get_receive_address,
            // Sync
            zstash_app_tauri_lib::commands::sync::zstash_start_sync,
            zstash_app_tauri_lib::commands::sync::zstash_stop_sync,
            zstash_app_tauri_lib::commands::sync::zstash_get_sync_progress,
            // Balance
            zstash_app_tauri_lib::commands::balance::zstash_get_balance,
            // Transactions
            zstash_app_tauri_lib::commands::transaction::zstash_list_transactions,
            zstash_app_tauri_lib::commands::transaction::zstash_prepare_send,
            zstash_app_tauri_lib::commands::transaction::zstash_confirm_send,
            zstash_app_tauri_lib::commands::transaction::zstash_cancel_send,
            zstash_app_tauri_lib::commands::transaction::zstash_retry_broadcast,
            zstash_app_tauri_lib::commands::transaction::zstash_shield_funds,
            // Backup
            zstash_app_tauri_lib::commands::backup::zstash_get_backup_challenge,
            zstash_app_tauri_lib::commands::backup::zstash_verify_backup,
            zstash_app_tauri_lib::commands::backup::zstash_restore_wallet,
            // Keystone
            zstash_app_tauri_lib::commands::keystone::zstash_import_ufvk,
            zstash_app_tauri_lib::commands::keystone::zstash_build_signing_request,
            zstash_app_tauri_lib::commands::keystone::zstash_finalize_signing,
            zstash_app_tauri_lib::commands::keystone::zstash_create_keystone_wallet,
            // Swaps
            zstash_app_tauri_lib::commands::swap::zstash_request_swap_quote,
            zstash_app_tauri_lib::commands::swap::zstash_start_swap,
            zstash_app_tauri_lib::commands::swap::zstash_get_swap_status,
            zstash_app_tauri_lib::commands::swap::zstash_list_swaps,
            // Tor
            zstash_app_tauri_lib::commands::tor::zstash_set_tor_enabled,
            zstash_app_tauri_lib::commands::tor::zstash_get_tor_state,
            // Logs
            zstash_app_tauri_lib::commands::logs::zstash_get_log_location,
            // Servers
            zstash_app_tauri_lib::commands::server::zstash_add_server,
            zstash_app_tauri_lib::commands::server::zstash_set_default_server,
            zstash_app_tauri_lib::commands::server::zstash_test_server,
            zstash_app_tauri_lib::commands::server::zstash_list_servers,
            // Version
            zstash_app_tauri_lib::commands::version::zstash_get_version,
            // Exchange Rate
            zstash_app_tauri_lib::commands::exchange_rate::zstash_get_fiat_settings,
            zstash_app_tauri_lib::commands::exchange_rate::zstash_set_fiat_settings,
            zstash_app_tauri_lib::commands::exchange_rate::zstash_get_exchange_rate,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
