// Compile-time guard: prevent test-bridge from being enabled in release builds.
// This feature exposes HTTP endpoints that return sensitive data (seed phrases).
#[cfg(all(feature = "test-bridge", not(debug_assertions)))]
compile_error!("test-bridge feature must not be enabled in release builds");

pub mod commands;
pub mod events;
pub mod state;
#[cfg(feature = "test-bridge")]
pub mod test_bridge;
pub mod windows;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // When test-bridge feature is enabled, run only the HTTP bridge server (no Tauri)
    #[cfg(feature = "test-bridge")]
    {
        run_test_bridge_only();
    }

    #[cfg(not(feature = "test-bridge"))]
    run_tauri_app();
}

#[cfg(not(feature = "test-bridge"))]
fn run_tauri_app() {
    let state = state::AppState::new().expect("failed to initialize application state");

    tauri::Builder::default()
        .manage(state)
        .plugin(tauri_plugin_opener::init())
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
            // Transactions
            commands::transaction::zstash_list_transactions,
            commands::transaction::zstash_prepare_send,
            commands::transaction::zstash_confirm_send,
            commands::transaction::zstash_cancel_send,
            commands::transaction::zstash_retry_broadcast,
            commands::transaction::zstash_shield_funds,
            // Backup
            commands::backup::zstash_get_backup_challenge,
            commands::backup::zstash_verify_backup,
            commands::backup::zstash_restore_wallet,
            // Keystone
            commands::keystone::zstash_import_ufvk,
            commands::keystone::zstash_build_signing_request,
            commands::keystone::zstash_finalize_signing,
            commands::keystone::zstash_create_keystone_wallet,
            // Swaps
            commands::swap::zstash_request_swap_quote,
            commands::swap::zstash_start_swap,
            commands::swap::zstash_get_swap_status,
            commands::swap::zstash_list_swaps,
            // Tor
            commands::tor::zstash_set_tor_enabled,
            commands::tor::zstash_get_tor_state,
            // Logs
            commands::logs::zstash_get_log_location,
            // Servers
            commands::server::zstash_add_server,
            commands::server::zstash_set_default_server,
            commands::server::zstash_test_server,
            commands::server::zstash_list_servers,
            // Version
            commands::version::zstash_get_version,
            // Exchange Rate
            commands::exchange_rate::zstash_get_fiat_settings,
            commands::exchange_rate::zstash_set_fiat_settings,
            commands::exchange_rate::zstash_get_exchange_rate,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

/// Run the test bridge HTTP server only (no Tauri webview).
///
/// This mode is used for E2E testing with Playwright/Chrome MCP.
/// The frontend is served by Vite and talks to this HTTP server.
#[cfg(feature = "test-bridge")]
pub fn run_test_bridge_only() {
    use std::sync::Arc;

    println!("Starting zstash in test-bridge mode...");

    let state = Arc::new(state::AppState::new().expect("failed to initialize application state"));

    let rt = tokio::runtime::Runtime::new().expect("failed to create tokio runtime");
    rt.block_on(async {
        if let Err(e) = test_bridge::start_test_bridge(state).await {
            eprintln!("Failed to start test bridge: {}", e);
            std::process::exit(1);
        }

        println!(
            "Test bridge server running on http://127.0.0.1:{}",
            test_bridge::TEST_BRIDGE_PORT
        );
        println!("Press Ctrl+C to stop");

        // Keep running indefinitely - the server will shutdown when the process exits
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(3600)).await;
        }
    });
}
