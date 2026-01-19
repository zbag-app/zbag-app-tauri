//! Native application menu for zSTASH.
//!
//! Provides app, file, wallet, security, network, and standard edit/window menus.

use tauri::{
    AppHandle, Emitter, Runtime,
    menu::{AboutMetadata, Menu, MenuItemBuilder, PredefinedMenuItem, Submenu},
};

/// Menu event channel names emitted to the frontend.
///
/// Keep in sync with `apps/zstash-app-tauri/src/constants/menuEvents.ts`.
pub mod events {
    pub const NEW_WALLET: &str = "menu:new-wallet";
    pub const RESTORE_WALLET: &str = "menu:restore-wallet";
    pub const SWITCH_WALLET: &str = "menu:switch-wallet";
    pub const LOCK_WALLET: &str = "menu:lock-wallet";
    pub const LOGOUT: &str = "menu:logout";
    pub const SEND: &str = "menu:send";
    pub const RECEIVE: &str = "menu:receive";
    pub const SWAP: &str = "menu:swap";
    pub const ACTIVITY: &str = "menu:activity";
    pub const SYNC_NOW: &str = "menu:sync-now";
    pub const STOP_SYNC: &str = "menu:stop-sync";
    pub const VIEW_SEED: &str = "menu:view-seed";
    pub const VERIFY_BACKUP: &str = "menu:verify-backup";
    pub const HARDWARE_WALLET: &str = "menu:hardware-wallet";
    pub const TOGGLE_TOR: &str = "menu:toggle-tor";
    pub const SERVER_SETTINGS: &str = "menu:server-settings";
    pub const PREFERENCES: &str = "menu:preferences";
    pub const OPEN_LOGS: &str = "menu:open-logs";
}

/// Build the application menu.
pub fn build_menu<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<Menu<R>> {
    let menu = Menu::new(app)?;

    // App menu (macOS only shows this as the app name menu)
    let app_menu = build_app_menu(app)?;
    menu.append(&app_menu)?;

    // File menu
    let file_menu = build_file_menu(app)?;
    menu.append(&file_menu)?;

    // Wallet menu
    let wallet_menu = build_wallet_menu(app)?;
    menu.append(&wallet_menu)?;

    // Security menu
    let security_menu = build_security_menu(app)?;
    menu.append(&security_menu)?;

    // Network menu
    let network_menu = build_network_menu(app)?;
    menu.append(&network_menu)?;

    // Edit menu (standard)
    let edit_menu = build_edit_menu(app)?;
    menu.append(&edit_menu)?;

    // Window menu (standard)
    let window_menu = build_window_menu(app)?;
    menu.append(&window_menu)?;

    // Help menu
    let help_menu = build_help_menu(app)?;
    menu.append(&help_menu)?;

    Ok(menu)
}

fn build_app_menu<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<Submenu<R>> {
    let about = PredefinedMenuItem::about(
        app,
        Some("About zSTASH"),
        Some(AboutMetadata {
            name: Some("zSTASH".into()),
            version: Some(env!("CARGO_PKG_VERSION").into()),
            ..Default::default()
        }),
    )?;

    let preferences = MenuItemBuilder::with_id("preferences", "Preferences...")
        .accelerator("CmdOrCtrl+,")
        .build(app)?;

    let quit = PredefinedMenuItem::quit(app, Some("Quit zSTASH"))?;
    let separator = PredefinedMenuItem::separator(app)?;

    #[cfg(target_os = "macos")]
    {
        let hide = PredefinedMenuItem::hide(app, Some("Hide zSTASH"))?;
        let hide_others = PredefinedMenuItem::hide_others(app, Some("Hide Others"))?;
        let show_all = PredefinedMenuItem::show_all(app, Some("Show All"))?;
        let separator2 = PredefinedMenuItem::separator(app)?;
        let separator3 = PredefinedMenuItem::separator(app)?;

        return Submenu::with_items(
            app,
            "zSTASH",
            true,
            &[
                &about,
                &separator,
                &preferences,
                &separator2,
                &hide,
                &hide_others,
                &show_all,
                &separator3,
                &quit,
            ],
        );
    }

    #[cfg(not(target_os = "macos"))]
    {
        return Submenu::with_items(
            app,
            "zSTASH",
            true,
            &[&about, &separator, &preferences, &quit],
        );
    }
}

fn build_file_menu<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<Submenu<R>> {
    let new_wallet = MenuItemBuilder::with_id("new_wallet", "New Wallet...")
        .accelerator("CmdOrCtrl+N")
        .build(app)?;

    let restore_wallet = MenuItemBuilder::with_id("restore_wallet", "Restore Wallet...")
        .accelerator("CmdOrCtrl+Shift+R")
        .build(app)?;

    let switch_wallet = MenuItemBuilder::with_id("switch_wallet", "Switch Wallet...")
        .accelerator("CmdOrCtrl+Shift+W")
        .build(app)?;

    let separator = PredefinedMenuItem::separator(app)?;

    let lock_wallet = MenuItemBuilder::with_id("lock_wallet", "Lock Wallet")
        .accelerator("CmdOrCtrl+L")
        .build(app)?;

    let logout = MenuItemBuilder::with_id("logout", "Logout").build(app)?;

    let separator2 = PredefinedMenuItem::separator(app)?;
    let close = PredefinedMenuItem::close_window(app, Some("Close Window"))?;

    Submenu::with_items(
        app,
        "File",
        true,
        &[
            &new_wallet,
            &restore_wallet,
            &switch_wallet,
            &separator,
            &lock_wallet,
            &logout,
            &separator2,
            &close,
        ],
    )
}

fn build_wallet_menu<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<Submenu<R>> {
    let send = MenuItemBuilder::with_id("send", "Send...")
        .accelerator("CmdOrCtrl+Shift+S")
        .build(app)?;

    let receive = MenuItemBuilder::with_id("receive", "Receive...")
        .accelerator("CmdOrCtrl+Shift+D")
        .build(app)?;

    let swap = MenuItemBuilder::with_id("swap", "Swap...")
        .accelerator("CmdOrCtrl+Shift+X")
        .build(app)?;

    let separator = PredefinedMenuItem::separator(app)?;

    let activity = MenuItemBuilder::with_id("activity", "Activity")
        .accelerator("CmdOrCtrl+Shift+A")
        .build(app)?;

    let separator2 = PredefinedMenuItem::separator(app)?;

    let sync_now = MenuItemBuilder::with_id("sync_now", "Sync Now")
        .accelerator("CmdOrCtrl+Shift+Y")
        .build(app)?;

    let stop_sync = MenuItemBuilder::with_id("stop_sync", "Stop Sync")
        .accelerator("CmdOrCtrl+.")
        .build(app)?;

    Submenu::with_items(
        app,
        "Wallet",
        true,
        &[
            &send,
            &receive,
            &swap,
            &separator,
            &activity,
            &separator2,
            &sync_now,
            &stop_sync,
        ],
    )
}

fn build_security_menu<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<Submenu<R>> {
    let view_seed = MenuItemBuilder::with_id("view_seed", "View Seed Phrase...").build(app)?;

    let verify_backup = MenuItemBuilder::with_id("verify_backup", "Verify Backup...").build(app)?;

    let separator = PredefinedMenuItem::separator(app)?;

    let hardware_wallet =
        MenuItemBuilder::with_id("hardware_wallet", "Hardware Wallet...").build(app)?;

    Submenu::with_items(
        app,
        "Security",
        true,
        &[&view_seed, &verify_backup, &separator, &hardware_wallet],
    )
}

fn build_network_menu<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<Submenu<R>> {
    let toggle_tor = MenuItemBuilder::with_id("toggle_tor", "Toggle Tor").build(app)?;

    let separator = PredefinedMenuItem::separator(app)?;

    let server_settings =
        MenuItemBuilder::with_id("server_settings", "Server Settings...").build(app)?;

    Submenu::with_items(
        app,
        "Network",
        true,
        &[&toggle_tor, &separator, &server_settings],
    )
}

fn build_edit_menu<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<Submenu<R>> {
    let undo = PredefinedMenuItem::undo(app, Some("Undo"))?;
    let redo = PredefinedMenuItem::redo(app, Some("Redo"))?;
    let separator = PredefinedMenuItem::separator(app)?;
    let cut = PredefinedMenuItem::cut(app, Some("Cut"))?;
    let copy = PredefinedMenuItem::copy(app, Some("Copy"))?;
    let paste = PredefinedMenuItem::paste(app, Some("Paste"))?;
    let select_all = PredefinedMenuItem::select_all(app, Some("Select All"))?;

    Submenu::with_items(
        app,
        "Edit",
        true,
        &[&undo, &redo, &separator, &cut, &copy, &paste, &select_all],
    )
}

fn build_window_menu<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<Submenu<R>> {
    let minimize = PredefinedMenuItem::minimize(app, Some("Minimize"))?;
    let zoom = PredefinedMenuItem::maximize(app, Some("Zoom"))?;
    let separator = PredefinedMenuItem::separator(app)?;
    let fullscreen = PredefinedMenuItem::fullscreen(app, Some("Enter Full Screen"))?;

    Submenu::with_items(
        app,
        "Window",
        true,
        &[&minimize, &zoom, &separator, &fullscreen],
    )
}

fn build_help_menu<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<Submenu<R>> {
    let open_logs = MenuItemBuilder::with_id("open_logs", "Open Logs Folder...").build(app)?;

    Submenu::with_items(app, "Help", true, &[&open_logs])
}

fn menu_id_to_event_name(menu_id: &str) -> Option<&'static str> {
    match menu_id {
        "new_wallet" => Some(events::NEW_WALLET),
        "restore_wallet" => Some(events::RESTORE_WALLET),
        "switch_wallet" => Some(events::SWITCH_WALLET),
        "lock_wallet" => Some(events::LOCK_WALLET),
        "logout" => Some(events::LOGOUT),
        "send" => Some(events::SEND),
        "receive" => Some(events::RECEIVE),
        "swap" => Some(events::SWAP),
        "activity" => Some(events::ACTIVITY),
        "sync_now" => Some(events::SYNC_NOW),
        "stop_sync" => Some(events::STOP_SYNC),
        "view_seed" => Some(events::VIEW_SEED),
        "verify_backup" => Some(events::VERIFY_BACKUP),
        "hardware_wallet" => Some(events::HARDWARE_WALLET),
        "toggle_tor" => Some(events::TOGGLE_TOR),
        "server_settings" => Some(events::SERVER_SETTINGS),
        "preferences" => Some(events::PREFERENCES),
        "open_logs" => Some(events::OPEN_LOGS),
        _ => None,
    }
}

/// Handle menu events and emit to the frontend.
pub fn handle_menu_event<R: Runtime>(app: &AppHandle<R>, event: &tauri::menu::MenuEvent) {
    let Some(event_name) = menu_id_to_event_name(event.id().as_ref()) else {
        #[cfg(debug_assertions)]
        tracing::debug!(menu_id = event.id().as_ref(), "unhandled menu event");
        return;
    };

    if let Err(err) = app.emit(event_name, ()) {
        tracing::warn!(error = ?err, event = event_name, "failed to emit menu event");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn menu_id_to_event_name_maps_known_ids() {
        let cases = [
            ("new_wallet", events::NEW_WALLET),
            ("restore_wallet", events::RESTORE_WALLET),
            ("switch_wallet", events::SWITCH_WALLET),
            ("lock_wallet", events::LOCK_WALLET),
            ("logout", events::LOGOUT),
            ("send", events::SEND),
            ("receive", events::RECEIVE),
            ("swap", events::SWAP),
            ("activity", events::ACTIVITY),
            ("sync_now", events::SYNC_NOW),
            ("stop_sync", events::STOP_SYNC),
            ("view_seed", events::VIEW_SEED),
            ("verify_backup", events::VERIFY_BACKUP),
            ("hardware_wallet", events::HARDWARE_WALLET),
            ("toggle_tor", events::TOGGLE_TOR),
            ("server_settings", events::SERVER_SETTINGS),
            ("preferences", events::PREFERENCES),
            ("open_logs", events::OPEN_LOGS),
        ];

        for (menu_id, expected_event_name) in cases {
            assert_eq!(
                menu_id_to_event_name(menu_id),
                Some(expected_event_name),
                "unexpected mapping for {menu_id}"
            );
        }
    }

    #[test]
    fn menu_id_to_event_name_returns_none_for_unknown_id() {
        assert_eq!(menu_id_to_event_name("not-a-real-menu-id"), None);
    }
}
