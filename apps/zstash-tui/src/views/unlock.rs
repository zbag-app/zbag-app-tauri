//! Password entry / unlock view.

use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Paragraph};

use crate::app::{App, AppAction, AppState, DashboardTab};
use crate::data::{derive_wallet_dek, open_app_db, open_wallet_db, wallet_db_path};
use crate::event::KeyAction;
use crate::ui::centered_rect;
use crate::ui::widgets::PasswordInput;

/// Render the unlock view
pub fn render(app: &App, frame: &mut Frame, area: Rect) {
    // Background with wallet name
    let wallet_name = app
        .selected_wallet()
        .map(|w| w.name.as_str())
        .unwrap_or("Unknown");

    let bg = Paragraph::new(format!("Unlocking: {}", wallet_name))
        .block(
            Block::default()
                .title(" zkore-tui ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Cyan)),
        )
        .alignment(Alignment::Center);
    frame.render_widget(bg, area);

    // Password input dialog
    let input_area = centered_rect(50, 30, area);
    PasswordInput::new(&app.password_input)
        .title(" Enter Password ")
        .error(app.error_message.as_deref())
        .render(frame, input_area);
}

/// Handle key input in unlock view
pub fn handle_key(app: &mut App, action: KeyAction) -> AppAction {
    match action {
        KeyAction::Back => {
            app.password_input.clear();
            app.clear_error();
            AppAction::ChangeState(AppState::WalletSelection)
        }
        KeyAction::Select => {
            // Try to unlock the wallet
            try_unlock(app)
        }
        KeyAction::Char(c) => {
            app.password_input.push(c);
            app.clear_error();
            AppAction::None
        }
        KeyAction::Backspace => {
            app.password_input.pop();
            app.clear_error();
            AppAction::None
        }
        _ => AppAction::None,
    }
}

fn try_unlock(app: &mut App) -> AppAction {
    let Some(wallet) = app.selected_wallet().cloned() else {
        app.set_error("No wallet selected");
        return AppAction::None;
    };

    // Try to derive the DEK
    let app_db = match open_app_db(&app.config.app_db_path) {
        Ok(db) => db,
        Err(e) => {
            app.set_error(format!("Failed to open app database: {}", e));
            return AppAction::None;
        }
    };

    let dek = match derive_wallet_dek(
        app_db.conn(),
        wallet.id,
        wallet.network,
        &app.password_input,
    ) {
        Ok(dek) => dek,
        Err(e) => {
            app.set_error(format!("Wrong password or error: {}", e));
            app.password_input.clear();
            return AppAction::None;
        }
    };

    // Cache the DEK bytes for later write operations (e.g., birthday editing)
    // Note: We copy the bytes before the DEK is potentially dropped
    let dek_bytes = dek.0;

    // Try to open the wallet database
    let db_path = wallet_db_path(&app.config.wallets_dir, wallet.network, wallet.id);
    match open_wallet_db(&db_path, &dek) {
        Ok(conn) => {
            // Load dashboard data from the wallet database
            match crate::views::dashboard::load_data(&conn, wallet.network) {
                Ok(dashboard_data) => {
                    app.dashboard_data = dashboard_data;
                }
                Err(e) => {
                    // Non-fatal: continue with empty dashboard data
                    app.set_error(format!("Warning: failed to load dashboard data: {}", e));
                    app.dashboard_data = crate::views::dashboard::DashboardData::default();
                }
            }

            // Success! Store the wallet info and DEK, then transition to dashboard
            app.current_wallet = Some(wallet);
            app.cached_dek_bytes = Some(dek_bytes);
            app.password_input.clear();
            // Clear error only if we didn't set a warning above
            if app.error_message.is_none()
                || !app
                    .error_message
                    .as_ref()
                    .is_some_and(|m| m.starts_with("Warning:"))
            {
                app.clear_error();
            }
            AppAction::ChangeState(AppState::Dashboard {
                tab: DashboardTab::Overview,
            })
        }
        Err(e) => {
            app.set_error(format!("Failed to open wallet: {}", e));
            app.password_input.clear();
            AppAction::None
        }
    }
}
