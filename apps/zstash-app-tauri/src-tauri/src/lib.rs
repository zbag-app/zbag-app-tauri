#![forbid(unsafe_code)]

// Compile-time guard: prevent test-bridge from being enabled in release builds.
// This feature exposes HTTP endpoints that return sensitive data (seed phrases).
#[cfg(all(feature = "test-bridge", not(debug_assertions)))]
compile_error!("test-bridge feature must not be enabled in release builds");

pub mod commands;
pub mod events;
pub mod exchange_logic;
pub mod menu;
pub mod server_logic;
pub mod state;
#[cfg(feature = "test-bridge")]
pub mod test_bridge;
pub mod time_utils;
pub mod wallet_logic;
pub mod windows;

#[cfg(all(
    not(feature = "test-bridge"),
    feature = "cef-runtime",
    target_os = "macos"
))]
use serde_json::{Map, Value};
#[cfg(not(feature = "test-bridge"))]
use std::sync::Arc;
#[cfg(all(
    not(feature = "test-bridge"),
    feature = "cef-runtime",
    target_os = "macos"
))]
use std::{fs, path::PathBuf};
#[cfg(not(feature = "test-bridge"))]
use tauri::Manager;

#[cfg(feature = "cef-runtime")]
type AppRuntime = tauri::Cef;
#[cfg(not(feature = "cef-runtime"))]
type AppRuntime = tauri::Wry;
type AppHandle = tauri::AppHandle<AppRuntime>;

#[cfg(all(not(feature = "test-bridge"), feature = "cef-runtime"))]
fn cef_runtime_args() -> Vec<(String, Option<String>)> {
    let mut args = Vec::new();

    #[cfg(target_os = "macos")]
    if std::env::var("ZSTASH_USE_SYSTEM_KEYCHAIN").as_deref() != Ok("1") {
        // POC default: avoid per-launch macOS keychain prompts from Chromium safe storage.
        args.push(("--use-mock-keychain".to_string(), None));
    }

    // Keep Chromium credential UI disabled so wallet auth remains app-controlled.
    args.push(("--disable-save-password-bubble".to_string(), None));

    args
}

#[cfg(all(
    not(feature = "test-bridge"),
    feature = "cef-runtime",
    target_os = "macos"
))]
fn cef_preferences_path(bundle_identifier: &str) -> Option<PathBuf> {
    let home = std::env::var_os("HOME")?;
    Some(
        PathBuf::from(home)
            .join("Library")
            .join("Caches")
            .join(bundle_identifier)
            .join("cef")
            .join("Default")
            .join("Preferences"),
    )
}

#[cfg(all(
    not(feature = "test-bridge"),
    feature = "cef-runtime",
    target_os = "macos"
))]
fn enforce_cef_password_policy(bundle_identifier: &str) {
    let Some(preferences_path) = cef_preferences_path(bundle_identifier) else {
        tracing::warn!("failed to locate HOME when applying CEF password policy");
        return;
    };

    if let Some(parent) = preferences_path.parent() {
        if let Err(error) = fs::create_dir_all(parent) {
            tracing::warn!(
                path = %parent.display(),
                ?error,
                "failed to prepare CEF preferences directory"
            );
            return;
        }
    }

    let mut root = match fs::read_to_string(&preferences_path) {
        Ok(raw) => {
            serde_json::from_str::<Value>(&raw).unwrap_or_else(|_| Value::Object(Map::new()))
        }
        Err(_) => Value::Object(Map::new()),
    };

    if !root.is_object() {
        root = Value::Object(Map::new());
    }

    let root_obj = root
        .as_object_mut()
        .expect("root should always be an object");
    root_obj.insert("credentials_enable_service".to_string(), Value::Bool(false));

    let profile = root_obj
        .entry("profile".to_string())
        .or_insert_with(|| Value::Object(Map::new()));
    if !profile.is_object() {
        *profile = Value::Object(Map::new());
    }
    if let Some(profile_obj) = profile.as_object_mut() {
        profile_obj.insert("password_manager_enabled".to_string(), Value::Bool(false));
        profile_obj.insert(
            "password_manager_leak_detection".to_string(),
            Value::Bool(false),
        );
    }

    let autofill = root_obj
        .entry("autofill".to_string())
        .or_insert_with(|| Value::Object(Map::new()));
    if !autofill.is_object() {
        *autofill = Value::Object(Map::new());
    }
    if let Some(autofill_obj) = autofill.as_object_mut() {
        autofill_obj.insert("enabled".to_string(), Value::Bool(false));
        autofill_obj.insert("profile_enabled".to_string(), Value::Bool(false));
        autofill_obj.insert("credit_card_enabled".to_string(), Value::Bool(false));
    }

    match serde_json::to_string(&root) {
        Ok(serialized) => {
            if let Err(error) = fs::write(&preferences_path, serialized) {
                tracing::warn!(
                    path = %preferences_path.display(),
                    ?error,
                    "failed to write CEF password policy preferences"
                );
            }
        }
        Err(error) => {
            tracing::warn!(
                path = %preferences_path.display(),
                ?error,
                "failed to serialize CEF password policy preferences"
            );
        }
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // When test-bridge feature is enabled, run only the HTTP bridge server (no Tauri)
    #[cfg(feature = "test-bridge")]
    {
        run_test_bridge_only();
    }

    // NOTE: Keep this command list in sync with `src-tauri/src/main.rs`.
    // The library entry point is used by tests/mobile contexts, while the binary
    // entry point uses its own handler registration.
    #[cfg(not(feature = "test-bridge"))]
    run_with_invoke_handler(tauri::generate_handler![
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
        // Jobs (async operations)
        commands::job::zstash_start_send_job,
        commands::job::zstash_start_shield_job,
        commands::job::zstash_cancel_job,
        commands::job::zstash_get_job_status,
        commands::job::zstash_list_jobs,
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
        commands::swap::zstash_get_supported_tokens,
        commands::swap::zstash_refresh_swap_status,
        commands::swap::zstash_resume_pending_swaps,
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
    ]);
}

#[cfg(not(feature = "test-bridge"))]
pub fn run_with_invoke_handler<F>(invoke_handler: F)
where
    F: Fn(tauri::ipc::Invoke<AppRuntime>) -> bool + Send + Sync + 'static,
{
    let state = state::AppState::new().expect("failed to initialize application state");

    // Log version at startup
    let version_info = zstash_core::version::VersionInfo::current();
    tracing::info!(
        version = %version_info.version,
        git_commit = version_info.git_commit.as_deref().unwrap_or("release"),
        build_timestamp = %version_info.build_timestamp,
        "zSTASH Desktop starting"
    );

    let context = tauri::generate_context!();

    #[cfg(all(feature = "cef-runtime", target_os = "macos"))]
    enforce_cef_password_policy(context.config().identifier.as_str());

    let builder = tauri::Builder::<AppRuntime>::default();

    #[cfg(feature = "cef-runtime")]
    let builder = {
        let cef_args = cef_runtime_args();
        if cef_args.is_empty() {
            builder
        } else {
            builder.command_line_args(cef_args)
        }
    };

    builder
        .manage(state)
        .menu(menu::build_menu)
        .on_menu_event(|app, event| menu::handle_menu_event(app, &event))
        .setup(|app| {
            let state = app.state::<state::AppState>();

            // Enter tokio runtime context so TorManager can spawn bootstrap task
            // Tauri's desktop async runtime is Tokio; if this ever changes the match will
            // become non-exhaustive at compile time rather than failing at runtime.
            let runtime_handle = tauri::async_runtime::handle();
            let _tokio_guard = match &runtime_handle {
                tauri::async_runtime::RuntimeHandle::Tokio(handle) => handle.enter(),
            };

            if let Err(err) = state.tor_manager.start_if_enabled() {
                tracing::warn!(error = ?err, "failed to start Tor on app launch");
            }

            let app_handle = app.handle().clone();

            {
                let wallet_manager = Arc::clone(&state.wallet_manager);
                let app_handle = app_handle.clone();
                if let Ok(mut mgr) = wallet_manager.lock() {
                    mgr.set_wallet_status_handler(Arc::new(move |event| {
                        let _ = events::emit_wallet_status(&app_handle, event);
                    }));
                }
            }

            let mut rx = state.tor_manager.subscribe();
            let tor_app = app_handle.clone();

            tauri::async_runtime::spawn(async move {
                let _ = events::emit_tor_status(&tor_app, rx.borrow().clone());
                while rx.changed().await.is_ok() {
                    let _ = events::emit_tor_status(&tor_app, rx.borrow().clone());
                }
            });

            Ok(())
        })
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(invoke_handler)
        .run(context)
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
