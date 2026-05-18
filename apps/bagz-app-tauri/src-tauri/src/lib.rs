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

#[cfg(all(not(feature = "test-bridge"), feature = "cef-runtime"))]
use serde_json::{Map, Value};
#[cfg(all(not(feature = "test-bridge"), feature = "cef-runtime"))]
use std::sync::Arc;
#[cfg(all(not(feature = "test-bridge"), feature = "cef-runtime"))]
use std::{fs, path::PathBuf};
#[cfg(all(not(feature = "test-bridge"), feature = "cef-runtime"))]
use tauri::Manager;

#[cfg(all(not(feature = "test-bridge"), feature = "cef-runtime"))]
type AppRuntime = tauri::Cef;

#[cfg(all(not(feature = "test-bridge"), feature = "cef-runtime"))]
pub const CEF_DISABLED_FEATURES: &str = concat!(
    "AutofillActorMode,",
    "AutofillServerCommunication,",
    "AsyncDns,",
    "DnsOverHttpsUpgrade,",
    "EnableMediaRouter,",
    "GlicActorUi,",
    "LensOverlay,",
    "LiveTranslate,",
    "MediaRouter,",
    "OptimizationGuideModelExecution,",
    "OptimizationGuideOnDeviceModel,",
    "OptimizationHints,",
    "PrivacySandboxSettings4,",
    "Translate,",
    "UseDnsHttpsSvcb"
);

#[cfg(all(not(feature = "test-bridge"), feature = "cef-runtime"))]
pub const CEF_HOST_RESOLVER_RULES: &str = concat!(
    "MAP * 0.0.0.0,",
    "EXCLUDE localhost,",
    "EXCLUDE 127.0.0.1,",
    "EXCLUDE ::1,",
    "EXCLUDE *.localhost,",
    "EXCLUDE ipc.localhost,",
    "EXCLUDE tauri.localhost"
);

#[cfg(all(not(feature = "test-bridge"), feature = "cef-runtime"))]
fn cef_switch(name: &str) -> (String, Option<String>) {
    (format!("--{name}"), None)
}

#[cfg(all(not(feature = "test-bridge"), feature = "cef-runtime"))]
fn cef_switch_value(name: &str, value: &str) -> (String, Option<String>) {
    (format!("--{name}"), Some(value.to_string()))
}

#[cfg(all(not(feature = "test-bridge"), feature = "cef-runtime"))]
pub fn cef_runtime_args() -> Vec<(String, Option<String>)> {
    let mut args = Vec::new();

    #[cfg(target_os = "macos")]
    if std::env::var("BAGZ_USE_SYSTEM_KEYCHAIN").as_deref() != Ok("1") {
        // POC default: avoid per-launch macOS keychain prompts from Chromium safe storage.
        args.push(cef_switch("use-mock-keychain"));
    }

    // Treat CEF as an offline renderer. All wallet networking belongs to Rust.
    // CEF_HARDENING_SWITCHES_BEGIN
    for switch in [
        "disable-background-networking",
        "disable-breakpad",
        "disable-component-extensions-with-background-pages",
        "disable-component-update",
        "disable-default-apps",
        "disable-domain-reliability",
        "disable-extensions",
        "disable-field-trial-config",
        "disable-notifications",
        "disable-print-preview",
        "disable-save-password-bubble",
        "disable-speech-api",
        "disable-sync",
        "disable-sync-invalidation-optimizations",
        "incognito",
        "metrics-recording-only",
        "no-default-browser-check",
        "no-first-run",
        "no-pings",
    ] {
        args.push(cef_switch(switch));
    }
    // CEF_HARDENING_SWITCHES_END

    // CEF_HARDENING_VALUED_ARGS_BEGIN
    args.push(cef_switch_value("disable-features", CEF_DISABLED_FEATURES));
    args.push(cef_switch_value("dns-over-https-mode", "off"));
    args.push(cef_switch_value("dns-over-https-templates", ""));
    args.push(cef_switch_value(
        "host-resolver-rules",
        CEF_HOST_RESOLVER_RULES,
    ));
    args.push(cef_switch_value(
        "webrtc-ip-handling-policy",
        "disable_non_proxied_udp",
    ));
    // CEF_HARDENING_VALUED_ARGS_END

    args
}

#[cfg(all(not(feature = "test-bridge"), feature = "cef-runtime"))]
fn default_cef_cache_path(bundle_identifier: &str) -> Option<PathBuf> {
    #[cfg(target_os = "macos")]
    {
        let home = std::env::var_os("HOME")?;
        return Some(
            PathBuf::from(home)
                .join("Library")
                .join("Caches")
                .join(bundle_identifier)
                .join("cef"),
        );
    }

    #[cfg(target_os = "windows")]
    {
        let local_app_data = std::env::var_os("LOCALAPPDATA")?;
        return Some(
            PathBuf::from(local_app_data)
                .join(bundle_identifier)
                .join("cef"),
        );
    }

    #[cfg(all(not(target_os = "macos"), not(target_os = "windows")))]
    {
        let base = std::env::var_os("XDG_CACHE_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".cache")))?;
        Some(base.join(bundle_identifier).join("cef"))
    }
}

#[cfg(all(not(feature = "test-bridge"), feature = "cef-runtime"))]
fn cef_runtime_cache_path(bundle_identifier: &str) -> PathBuf {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();
    std::env::temp_dir().join("bagz-cef").join(format!(
        "{bundle_identifier}-{}-{stamp}",
        std::process::id()
    ))
}

#[cfg(all(not(feature = "test-bridge"), feature = "cef-runtime"))]
fn purge_legacy_cef_cache(bundle_identifier: &str, runtime_cache_path: &PathBuf) {
    let Some(default_cache_path) = default_cef_cache_path(bundle_identifier) else {
        tracing::warn!("failed to locate default CEF cache path");
        return;
    };

    if default_cache_path == *runtime_cache_path || !default_cache_path.exists() {
        return;
    }

    if let Err(error) = fs::remove_dir_all(&default_cache_path) {
        tracing::warn!(
            path = %default_cache_path.display(),
            ?error,
            "failed to remove legacy persistent CEF cache"
        );
    }
}

#[cfg(all(not(feature = "test-bridge"), feature = "cef-runtime"))]
fn purge_stale_temp_cef_caches(bundle_identifier: &str, runtime_cache_path: &PathBuf) {
    let temp_cef_root = std::env::temp_dir().join("bagz-cef");
    let Ok(entries) = fs::read_dir(&temp_cef_root) else {
        return;
    };
    let prefix = format!("{bundle_identifier}-");
    let now = std::time::SystemTime::now();
    let min_age = std::time::Duration::from_secs(60);

    for entry in entries.flatten() {
        let path = entry.path();
        if path == *runtime_cache_path {
            continue;
        }

        let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        if !name.starts_with(&prefix) {
            continue;
        }

        let remainder = &name[prefix.len()..];
        let pid_alive = remainder
            .split_once('-')
            .and_then(|(pid_str, _)| pid_str.parse::<u32>().ok())
            .map(is_process_alive)
            .unwrap_or(false);
        if pid_alive {
            continue;
        }

        let metadata = match fs::metadata(&path) {
            Ok(metadata) => metadata,
            Err(_) => continue,
        };
        let modified = match metadata.modified() {
            Ok(modified) => modified,
            Err(_) => continue,
        };
        let age = match now.duration_since(modified) {
            Ok(age) => age,
            Err(_) => continue,
        };
        if age < min_age {
            continue;
        }

        if let Err(error) = fs::remove_dir_all(&path) {
            tracing::warn!(
                path = %path.display(),
                ?error,
                "failed to remove stale temp CEF cache"
            );
        }
    }
}

#[cfg(all(not(feature = "test-bridge"), feature = "cef-runtime"))]
fn is_process_alive(pid: u32) -> bool {
    #[cfg(unix)]
    {
        std::process::Command::new("/bin/kill")
            .args(["-0", &pid.to_string()])
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .map(|status| status.success())
            .unwrap_or(true)
    }
    #[cfg(not(unix))]
    {
        let _ = pid;
        true
    }
}

#[cfg(all(not(feature = "test-bridge"), feature = "cef-runtime"))]
fn object_entry<'a>(object: &'a mut Map<String, Value>, key: &str) -> &'a mut Map<String, Value> {
    let value = object
        .entry(key.to_string())
        .or_insert_with(|| Value::Object(Map::new()));
    if !value.is_object() {
        *value = Value::Object(Map::new());
    }
    value
        .as_object_mut()
        .expect("CEF preference entry should be an object")
}

#[cfg(all(not(feature = "test-bridge"), feature = "cef-runtime"))]
fn enforce_cef_browser_policy(cache_path: &PathBuf) {
    let preferences_path = cache_path.join("Default").join("Preferences");

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

    // CEF_HARDENING_PREFS_BEGIN
    for stale_key in [
        "account_tracker_service_last_update",
        "commerce_daily_metrics_last_update_time",
        "domain_diversity",
        "enterprise_profile_guid",
        "gaia_cookie",
        "gcm",
        "google",
        "invalidation",
        "media_router",
        "optimization_guide",
        "segmentation_platform",
        "webauthn",
    ] {
        root_obj.remove(stale_key);
    }

    root_obj.insert("credentials_enable_service".to_string(), Value::Bool(false));
    root_obj.insert("enable_do_not_track".to_string(), Value::Bool(true));
    root_obj.insert(
        "default_apps_install_state".to_string(),
        Value::Number(3.into()),
    );

    object_entry(root_obj, "alternate_error_pages")
        .insert("enabled".to_string(), Value::Bool(false));

    let autofill_obj = object_entry(root_obj, "autofill");
    autofill_obj.insert("enabled".to_string(), Value::Bool(false));
    autofill_obj.insert("profile_enabled".to_string(), Value::Bool(false));
    autofill_obj.insert("credit_card_enabled".to_string(), Value::Bool(false));

    object_entry(root_obj, "browser")
        .insert("enable_spellchecking".to_string(), Value::Bool(false));

    let dns_over_https_obj = object_entry(root_obj, "dns_over_https");
    dns_over_https_obj.insert("mode".to_string(), Value::String("off".to_string()));
    dns_over_https_obj.insert("templates".to_string(), Value::String(String::new()));
    dns_over_https_obj.insert(
        "automatic_mode_fallback_to_doh".to_string(),
        Value::Bool(false),
    );

    object_entry(root_obj, "net").insert(
        "network_prediction_options".to_string(),
        Value::Number(2.into()),
    );

    let privacy_sandbox_obj = object_entry(root_obj, "privacy_sandbox");
    privacy_sandbox_obj.insert("first_party_sets_enabled".to_string(), Value::Bool(false));
    let privacy_sandbox_m1_obj = object_entry(privacy_sandbox_obj, "m1");
    privacy_sandbox_m1_obj.insert("topics_enabled".to_string(), Value::Bool(false));
    privacy_sandbox_m1_obj.insert("fledge_enabled".to_string(), Value::Bool(false));
    privacy_sandbox_m1_obj.insert("ad_measurement_enabled".to_string(), Value::Bool(false));

    let profile_obj = object_entry(root_obj, "profile");
    profile_obj.insert("password_manager_enabled".to_string(), Value::Bool(false));
    profile_obj.insert(
        "password_manager_leak_detection".to_string(),
        Value::Bool(false),
    );
    profile_obj.insert(
        "network_prediction_options".to_string(),
        Value::Number(2.into()),
    );

    let safebrowsing_obj = object_entry(root_obj, "safebrowsing");
    safebrowsing_obj.insert("enabled".to_string(), Value::Bool(false));
    safebrowsing_obj.insert("enhanced".to_string(), Value::Bool(false));
    safebrowsing_obj.insert("scout_reporting_enabled".to_string(), Value::Bool(false));
    safebrowsing_obj.insert(
        "scout_reporting_enabled_when_deprecated".to_string(),
        Value::Bool(false),
    );
    safebrowsing_obj.insert("deep_scanning_enabled".to_string(), Value::Bool(false));
    safebrowsing_obj.insert("surveys_enabled".to_string(), Value::Bool(false));

    object_entry(root_obj, "search").insert("suggest_enabled".to_string(), Value::Bool(false));
    object_entry(root_obj, "signin").insert("allowed".to_string(), Value::Bool(false));

    let spellcheck_obj = object_entry(root_obj, "spellcheck");
    spellcheck_obj.insert("dictionaries".to_string(), Value::Array(Vec::new()));
    spellcheck_obj.insert("use_spelling_service".to_string(), Value::Bool(false));

    object_entry(root_obj, "sync").insert("requested".to_string(), Value::Bool(false));
    object_entry(root_obj, "translate").insert("enabled".to_string(), Value::Bool(false));

    let webrtc_obj = object_entry(root_obj, "webrtc");
    webrtc_obj.insert(
        "ip_handling_policy".to_string(),
        Value::String("disable_non_proxied_udp".to_string()),
    );
    webrtc_obj.insert("multiple_routes_enabled".to_string(), Value::Bool(false));
    webrtc_obj.insert("nonproxied_udp_enabled".to_string(), Value::Bool(false));
    // CEF_HARDENING_PREFS_END

    match serde_json::to_string(&root) {
        Ok(serialized) => {
            if let Err(error) = fs::write(&preferences_path, serialized) {
                tracing::warn!(
                    path = %preferences_path.display(),
                    ?error,
                    "failed to write CEF browser policy preferences"
                );
            }
        }
        Err(error) => {
            tracing::warn!(
                path = %preferences_path.display(),
                ?error,
                "failed to serialize CEF browser policy preferences"
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
    #[cfg(all(not(feature = "test-bridge"), feature = "cef-runtime"))]
    run_with_invoke_handler(tauri::generate_handler![
        // Wallet
        commands::wallet::bagz_create_wallet,
        commands::wallet::bagz_load_wallet,
        commands::wallet::bagz_list_wallets,
        commands::wallet::bagz_get_wallet_status,
        commands::wallet::bagz_unlock_wallet,
        commands::wallet::bagz_lock_wallet,
        commands::wallet::bagz_reauth_wallet,
        commands::wallet::bagz_view_seed_phrase,
        commands::wallet::bagz_logout_wallet,
        // Address
        commands::address::bagz_get_receive_address,
        // Sync
        commands::sync::bagz_start_sync,
        commands::sync::bagz_stop_sync,
        commands::sync::bagz_get_sync_progress,
        // Balance
        commands::balance::bagz_get_balance,
        // Transactions
        commands::transaction::bagz_list_transactions,
        commands::transaction::bagz_prepare_send,
        commands::transaction::bagz_confirm_send,
        commands::transaction::bagz_cancel_send,
        commands::transaction::bagz_retry_broadcast,
        commands::transaction::bagz_shield_funds,
        // Jobs (async operations)
        commands::job::bagz_start_send_job,
        commands::job::bagz_start_shield_job,
        commands::job::bagz_cancel_job,
        commands::job::bagz_get_job_status,
        commands::job::bagz_list_jobs,
        // Backup
        commands::backup::bagz_get_backup_challenge,
        commands::backup::bagz_verify_backup,
        commands::backup::bagz_restore_wallet,
        // Keystone
        commands::keystone::bagz_import_ufvk,
        commands::keystone::bagz_build_signing_request,
        commands::keystone::bagz_finalize_signing,
        commands::keystone::bagz_create_keystone_wallet,
        // Swaps
        commands::swap::bagz_request_swap_quote,
        commands::swap::bagz_start_swap,
        commands::swap::bagz_get_swap_status,
        commands::swap::bagz_list_swaps,
        commands::swap::bagz_get_supported_tokens,
        commands::swap::bagz_refresh_swap_status,
        commands::swap::bagz_resume_pending_swaps,
        // Tor
        commands::tor::bagz_set_tor_enabled,
        commands::tor::bagz_get_tor_state,
        // Logs
        commands::logs::bagz_get_log_location,
        // Servers
        commands::server::bagz_add_server,
        commands::server::bagz_set_default_server,
        commands::server::bagz_test_server,
        commands::server::bagz_list_servers,
        // Version
        commands::version::bagz_get_version,
        // Exchange Rate
        commands::exchange_rate::bagz_get_fiat_settings,
        commands::exchange_rate::bagz_set_fiat_settings,
        commands::exchange_rate::bagz_get_exchange_rate,
    ]);
}

#[cfg(all(not(feature = "test-bridge"), feature = "cef-runtime"))]
pub fn run_with_invoke_handler<F>(invoke_handler: F)
where
    F: Fn(tauri::ipc::Invoke<AppRuntime>) -> bool + Send + Sync + 'static,
{
    // Install ring before AppState, network, Tor, or any TLS use.
    bagz_network::install_ring_crypto_provider();

    let state = state::AppState::new().expect("failed to initialize application state");

    // Log version at startup
    let version_info = bagz_core::version::VersionInfo::current();
    tracing::info!(
        version = %version_info.version,
        git_commit = version_info.git_commit.as_deref().unwrap_or("release"),
        build_timestamp = %version_info.build_timestamp,
        "bagZ Desktop starting"
    );

    let context = tauri::generate_context!();

    #[cfg(all(feature = "cef-runtime", target_os = "macos"))]
    let bundle_identifier = context.config().identifier.as_str();
    #[cfg(all(feature = "cef-runtime", not(target_os = "macos")))]
    let bundle_identifier = context.config().identifier.as_str();

    #[cfg(feature = "cef-runtime")]
    let cef_cache_path = cef_runtime_cache_path(bundle_identifier);

    #[cfg(feature = "cef-runtime")]
    {
        purge_legacy_cef_cache(bundle_identifier, &cef_cache_path);
        purge_stale_temp_cef_caches(bundle_identifier, &cef_cache_path);
        enforce_cef_browser_policy(&cef_cache_path);
    }

    let builder = tauri::Builder::<AppRuntime>::default();

    #[cfg(feature = "cef-runtime")]
    let builder = {
        let cef_args = cef_runtime_args();
        if cef_args.is_empty() {
            builder.root_cache_path(&cef_cache_path)
        } else {
            builder
                .root_cache_path(&cef_cache_path)
                .command_line_args(cef_args)
        }
    };

    let run_result = builder
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

            #[cfg(feature = "cef-runtime")]
            if std::env::var("BAGZ_HEADLESS_SMOKE").is_ok() {
                let duration_secs = std::env::var("BAGZ_SMOKE_DURATION_SECS")
                    .ok()
                    .and_then(|raw| raw.parse::<u64>().ok())
                    .filter(|duration| *duration > 0)
                    .unwrap_or(15);

                if let Ok(path) = std::env::var("BAGZ_SMOKE_READY_FILE") {
                    if let Err(error) = fs::write(&path, "1") {
                        tracing::warn!(
                            path = %path,
                            ?error,
                            "failed to write CEF smoke ready sentinel"
                        );
                    }
                }

                let smoke_app = app.handle().clone();
                tauri::async_runtime::spawn(async move {
                    tokio::time::sleep(std::time::Duration::from_secs(duration_secs)).await;
                    smoke_app.exit(0);
                });
            }

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
        .run(context);

    #[cfg(feature = "cef-runtime")]
    if let Err(error) = fs::remove_dir_all(&cef_cache_path) {
        tracing::warn!(
            path = %cef_cache_path.display(),
            ?error,
            "failed to remove temp CEF cache"
        );
    }

    run_result.expect("error while running tauri application");
}

/// Run the test bridge HTTP server only (no Tauri webview).
///
/// This mode is used for E2E testing with Playwright/Chrome MCP.
/// The frontend is served by Vite and talks to this HTTP server.
#[cfg(feature = "test-bridge")]
pub fn run_test_bridge_only() {
    use std::sync::Arc;

    println!("Starting bagz in test-bridge mode...");

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
