#![forbid(unsafe_code)]
// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

#[cfg_attr(feature = "cef-runtime", tauri::cef_entry_point)]
fn main() {
    #[cfg(feature = "test-bridge")]
    {
        zbag_app_tauri_lib::run();
    }

    // NOTE: Keep this command list in sync with `src-tauri/src/lib.rs`.
    // The binary uses this registration path, while the library has its own
    // entry point for other runtime contexts.
    #[cfg(all(not(feature = "test-bridge"), feature = "cef-runtime"))]
    {
        zbag_app_tauri_lib::run_with_invoke_handler(tauri::generate_handler![
            // Wallet
            zbag_app_tauri_lib::commands::wallet::zbag_create_wallet,
            zbag_app_tauri_lib::commands::wallet::zbag_load_wallet,
            zbag_app_tauri_lib::commands::wallet::zbag_list_wallets,
            zbag_app_tauri_lib::commands::wallet::zbag_get_wallet_status,
            zbag_app_tauri_lib::commands::wallet::zbag_unlock_wallet,
            zbag_app_tauri_lib::commands::wallet::zbag_lock_wallet,
            zbag_app_tauri_lib::commands::wallet::zbag_reauth_wallet,
            zbag_app_tauri_lib::commands::wallet::zbag_view_seed_phrase,
            zbag_app_tauri_lib::commands::wallet::zbag_logout_wallet,
            // Address
            zbag_app_tauri_lib::commands::address::zbag_get_receive_address,
            // Sync
            zbag_app_tauri_lib::commands::sync::zbag_start_sync,
            zbag_app_tauri_lib::commands::sync::zbag_stop_sync,
            zbag_app_tauri_lib::commands::sync::zbag_get_sync_progress,
            // Balance
            zbag_app_tauri_lib::commands::balance::zbag_get_balance,
            // Transactions
            zbag_app_tauri_lib::commands::transaction::zbag_list_transactions,
            zbag_app_tauri_lib::commands::transaction::zbag_prepare_send,
            zbag_app_tauri_lib::commands::transaction::zbag_confirm_send,
            zbag_app_tauri_lib::commands::transaction::zbag_cancel_send,
            zbag_app_tauri_lib::commands::transaction::zbag_retry_broadcast,
            zbag_app_tauri_lib::commands::transaction::zbag_shield_funds,
            // Jobs (async operations)
            zbag_app_tauri_lib::commands::job::zbag_start_send_job,
            zbag_app_tauri_lib::commands::job::zbag_start_shield_job,
            zbag_app_tauri_lib::commands::job::zbag_cancel_job,
            zbag_app_tauri_lib::commands::job::zbag_get_job_status,
            zbag_app_tauri_lib::commands::job::zbag_list_jobs,
            // Backup
            zbag_app_tauri_lib::commands::backup::zbag_get_backup_challenge,
            zbag_app_tauri_lib::commands::backup::zbag_verify_backup,
            zbag_app_tauri_lib::commands::backup::zbag_restore_wallet,
            // Keystone
            zbag_app_tauri_lib::commands::keystone::zbag_import_ufvk,
            zbag_app_tauri_lib::commands::keystone::zbag_build_signing_request,
            zbag_app_tauri_lib::commands::keystone::zbag_finalize_signing,
            zbag_app_tauri_lib::commands::keystone::zbag_create_keystone_wallet,
            // Swaps
            zbag_app_tauri_lib::commands::swap::zbag_request_swap_quote,
            zbag_app_tauri_lib::commands::swap::zbag_start_swap,
            zbag_app_tauri_lib::commands::swap::zbag_get_swap_status,
            zbag_app_tauri_lib::commands::swap::zbag_list_swaps,
            zbag_app_tauri_lib::commands::swap::zbag_get_supported_tokens,
            zbag_app_tauri_lib::commands::swap::zbag_refresh_swap_status,
            zbag_app_tauri_lib::commands::swap::zbag_resume_pending_swaps,
            // Tor
            zbag_app_tauri_lib::commands::tor::zbag_set_tor_enabled,
            zbag_app_tauri_lib::commands::tor::zbag_get_tor_state,
            // Logs
            zbag_app_tauri_lib::commands::logs::zbag_get_log_location,
            // Servers
            zbag_app_tauri_lib::commands::server::zbag_add_server,
            zbag_app_tauri_lib::commands::server::zbag_set_default_server,
            zbag_app_tauri_lib::commands::server::zbag_test_server,
            zbag_app_tauri_lib::commands::server::zbag_list_servers,
            // Version
            zbag_app_tauri_lib::commands::version::zbag_get_version,
            // Exchange Rate
            zbag_app_tauri_lib::commands::exchange_rate::zbag_get_fiat_settings,
            zbag_app_tauri_lib::commands::exchange_rate::zbag_set_fiat_settings,
            zbag_app_tauri_lib::commands::exchange_rate::zbag_get_exchange_rate,
        ]);
    }
}
