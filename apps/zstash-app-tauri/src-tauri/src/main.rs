// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    #[cfg(feature = "test-bridge")]
    {
        zstash_app_tauri_lib::run();
    }

    // NOTE: Keep this command list in sync with `src-tauri/src/lib.rs`.
    // The binary uses this registration path, while the library has its own
    // entry point for other runtime contexts.
    #[cfg(not(feature = "test-bridge"))]
    {
        zstash_app_tauri_lib::run_with_invoke_handler(tauri::generate_handler![
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
            // Jobs (async operations)
            zstash_app_tauri_lib::commands::job::zstash_start_send_job,
            zstash_app_tauri_lib::commands::job::zstash_start_shield_job,
            zstash_app_tauri_lib::commands::job::zstash_cancel_job,
            zstash_app_tauri_lib::commands::job::zstash_get_job_status,
            zstash_app_tauri_lib::commands::job::zstash_list_jobs,
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
            zstash_app_tauri_lib::commands::swap::zstash_get_supported_tokens,
            zstash_app_tauri_lib::commands::swap::zstash_refresh_swap_status,
            zstash_app_tauri_lib::commands::swap::zstash_resume_pending_swaps,
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
        ]);
    }
}
