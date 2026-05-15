#![forbid(unsafe_code)]
// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

#[cfg_attr(feature = "cef-runtime", tauri::cef_entry_point)]
fn main() {
    #[cfg(feature = "test-bridge")]
    {
        bagz_app_tauri_lib::run();
    }

    // NOTE: Keep this command list in sync with `src-tauri/src/lib.rs`.
    // The binary uses this registration path, while the library has its own
    // entry point for other runtime contexts.
    #[cfg(all(not(feature = "test-bridge"), feature = "cef-runtime"))]
    {
        bagz_app_tauri_lib::run_with_invoke_handler(tauri::generate_handler![
            // Wallet
            bagz_app_tauri_lib::commands::wallet::bagz_create_wallet,
            bagz_app_tauri_lib::commands::wallet::bagz_load_wallet,
            bagz_app_tauri_lib::commands::wallet::bagz_list_wallets,
            bagz_app_tauri_lib::commands::wallet::bagz_get_wallet_status,
            bagz_app_tauri_lib::commands::wallet::bagz_unlock_wallet,
            bagz_app_tauri_lib::commands::wallet::bagz_lock_wallet,
            bagz_app_tauri_lib::commands::wallet::bagz_reauth_wallet,
            bagz_app_tauri_lib::commands::wallet::bagz_view_seed_phrase,
            bagz_app_tauri_lib::commands::wallet::bagz_logout_wallet,
            // Address
            bagz_app_tauri_lib::commands::address::bagz_get_receive_address,
            // Sync
            bagz_app_tauri_lib::commands::sync::bagz_start_sync,
            bagz_app_tauri_lib::commands::sync::bagz_stop_sync,
            bagz_app_tauri_lib::commands::sync::bagz_get_sync_progress,
            // Balance
            bagz_app_tauri_lib::commands::balance::bagz_get_balance,
            // Transactions
            bagz_app_tauri_lib::commands::transaction::bagz_list_transactions,
            bagz_app_tauri_lib::commands::transaction::bagz_prepare_send,
            bagz_app_tauri_lib::commands::transaction::bagz_confirm_send,
            bagz_app_tauri_lib::commands::transaction::bagz_cancel_send,
            bagz_app_tauri_lib::commands::transaction::bagz_retry_broadcast,
            bagz_app_tauri_lib::commands::transaction::bagz_shield_funds,
            // Jobs (async operations)
            bagz_app_tauri_lib::commands::job::bagz_start_send_job,
            bagz_app_tauri_lib::commands::job::bagz_start_shield_job,
            bagz_app_tauri_lib::commands::job::bagz_cancel_job,
            bagz_app_tauri_lib::commands::job::bagz_get_job_status,
            bagz_app_tauri_lib::commands::job::bagz_list_jobs,
            // Backup
            bagz_app_tauri_lib::commands::backup::bagz_get_backup_challenge,
            bagz_app_tauri_lib::commands::backup::bagz_verify_backup,
            bagz_app_tauri_lib::commands::backup::bagz_restore_wallet,
            // Keystone
            bagz_app_tauri_lib::commands::keystone::bagz_import_ufvk,
            bagz_app_tauri_lib::commands::keystone::bagz_build_signing_request,
            bagz_app_tauri_lib::commands::keystone::bagz_finalize_signing,
            bagz_app_tauri_lib::commands::keystone::bagz_create_keystone_wallet,
            // Swaps
            bagz_app_tauri_lib::commands::swap::bagz_request_swap_quote,
            bagz_app_tauri_lib::commands::swap::bagz_start_swap,
            bagz_app_tauri_lib::commands::swap::bagz_get_swap_status,
            bagz_app_tauri_lib::commands::swap::bagz_list_swaps,
            bagz_app_tauri_lib::commands::swap::bagz_get_supported_tokens,
            bagz_app_tauri_lib::commands::swap::bagz_refresh_swap_status,
            bagz_app_tauri_lib::commands::swap::bagz_resume_pending_swaps,
            // Tor
            bagz_app_tauri_lib::commands::tor::bagz_set_tor_enabled,
            bagz_app_tauri_lib::commands::tor::bagz_get_tor_state,
            // Logs
            bagz_app_tauri_lib::commands::logs::bagz_get_log_location,
            // Servers
            bagz_app_tauri_lib::commands::server::bagz_add_server,
            bagz_app_tauri_lib::commands::server::bagz_set_default_server,
            bagz_app_tauri_lib::commands::server::bagz_test_server,
            bagz_app_tauri_lib::commands::server::bagz_list_servers,
            // Version
            bagz_app_tauri_lib::commands::version::bagz_get_version,
            // Exchange Rate
            bagz_app_tauri_lib::commands::exchange_rate::bagz_get_fiat_settings,
            bagz_app_tauri_lib::commands::exchange_rate::bagz_set_fiat_settings,
            bagz_app_tauri_lib::commands::exchange_rate::bagz_get_exchange_rate,
        ]);
    }
}
