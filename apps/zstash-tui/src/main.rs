//! # zkore-tui
//!
//! A terminal user interface for inspecting and managing Zkore wallet data.
//!
//! ## Features
//!
//! - View all wallets and their details
//! - Inspect account information including birthday height
//! - Modify birthday height (with confirmation)
//! - Reset sync state
//! - Edit wallet settings
//!
//! ## Usage
//!
//! ```bash
//! cargo run -p zkore-tui
//! # Or with custom paths:
//! cargo run -p zkore-tui -- --app-db /path/to/app.db --wallets-dir /path/to/wallets
//! ```
//!
//! ## Keybindings
//!
//! - `q` - Quit
//! - `?` - Show help
//! - `Tab/Shift+Tab` - Switch tabs
//! - `j/k` or arrow keys - Navigate
//! - `Enter` - Select/Confirm
//! - `Esc` - Go back

use std::io;

use anyhow::Result;
use clap::Parser;
use crossterm::{
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{
    prelude::*,
    widgets::{Block, Borders, Paragraph},
};

mod app;
mod data;
mod error;
mod event;
mod ui;
mod views;

use crate::app::{AppAction, AppState};
use crate::event::{EventResult, KeyAction, poll_event};
use crate::ui::widgets::HelpOverlay;

#[derive(Parser, Debug)]
#[command(name = "zkore-tui")]
#[command(about = "TUI tool for inspecting and managing Zkore wallet data")]
struct Args {
    /// Path to the app database (defaults to ~/.zkore/app.db)
    #[arg(long)]
    app_db: Option<std::path::PathBuf>,

    /// Path to the wallets directory (defaults to ~/.zkore/wallets)
    #[arg(long)]
    wallets_dir: Option<std::path::PathBuf>,
}

fn main() -> Result<()> {
    let args = Args::parse();

    let config = app::AppConfig::new(
        args.app_db
            .unwrap_or_else(|| default_app_db_path().expect("HOME not set")),
        args.wallets_dir
            .unwrap_or_else(|| default_wallets_dir().expect("HOME not set")),
    );

    // Setup panic hook to restore terminal
    let original_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic_info| {
        let _ = restore_terminal();
        original_hook(panic_info);
    }));

    // Initialize terminal
    let mut terminal = setup_terminal()?;

    // Run the app
    let result = run(&mut terminal, config);

    // Restore terminal
    restore_terminal()?;

    result
}

fn setup_terminal() -> Result<Terminal<CrosstermBackend<io::Stdout>>> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let terminal = Terminal::new(backend)?;
    Ok(terminal)
}

fn restore_terminal() -> Result<()> {
    disable_raw_mode()?;
    execute!(io::stdout(), LeaveAlternateScreen)?;
    Ok(())
}

fn run(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    config: app::AppConfig,
) -> Result<()> {
    let mut app = app::App::new(config);

    // Load wallets on startup
    if let Err(e) = load_wallets(&mut app) {
        app.set_error(format!("Failed to load wallets: {}", e));
    }

    loop {
        // Render based on current state
        terminal.draw(|frame| {
            let area = frame.area();
            render_view(&mut app, frame, area);
        })?;

        // Handle events
        match poll_event(std::time::Duration::from_millis(100))? {
            EventResult::Key(action) => {
                let app_action = handle_key(&mut app, action);
                match app_action {
                    AppAction::Quit => return Ok(()),
                    AppAction::ChangeState(new_state) => app.go_to(new_state),
                    AppAction::ShowError(msg) => app.set_error(msg),
                    AppAction::None => {}
                }
            }
            EventResult::Resize(_, _) => {
                // Terminal will redraw on next iteration
            }
            EventResult::Tick => {}
        }

        if app.should_quit {
            return Ok(());
        }
    }
}

fn load_wallets(app: &mut app::App) -> Result<()> {
    let app_db = data::open_app_db(&app.config.app_db_path)?;
    app.wallets = data::list_wallets(&app_db)?;
    Ok(())
}

fn render_view(app: &mut app::App, frame: &mut Frame, area: Rect) {
    // Split area for main content and status bar
    let chunks = Layout::vertical([Constraint::Min(0), Constraint::Length(1)]).split(area);
    let main_area = chunks[0];
    let status_area = chunks[1];

    // Render the main view
    match app.state.clone() {
        AppState::WalletSelection => {
            views::wallet_selector::render(app, frame, main_area);
        }
        AppState::PasswordEntry => {
            views::unlock::render(app, frame, main_area);
        }
        AppState::Dashboard { tab } => {
            views::dashboard::render(app, frame, main_area, tab, &app.dashboard_data);
        }
        AppState::AccountDetails { account_id } => {
            views::account_details::render(app, frame, main_area, account_id);
        }
        AppState::BirthdayEditor { account_id } => {
            views::birthday_editor::render(app, frame, main_area, account_id);
        }
        AppState::SyncReset => {
            views::sync_reset::render(app, frame, main_area, &app.sync_reset_state);
        }
        AppState::Settings => {
            views::settings::render(app, frame, main_area, &app.settings_state);
        }
        _ => {
            render_placeholder(frame, main_area, &format!("{:?}", app.state));
        }
    }

    // Render the status bar
    let wallet_name = app.current_wallet.as_ref().map(|w| w.name.as_str());
    let network = app
        .current_wallet
        .as_ref()
        .map(|w| format!("{:?}", w.network));
    ui::render_status_bar(frame, status_area, wallet_name, network.as_deref());

    // Render help overlay if shown (must be last to appear on top)
    if app.show_help {
        HelpOverlay::render(frame, area);
    }
}

fn handle_key(app: &mut app::App, action: KeyAction) -> AppAction {
    // If help overlay is shown, any key closes it
    if app.show_help {
        app.show_help = false;
        return AppAction::None;
    }

    // Show help on '?' (except in text input states)
    if matches!(action, KeyAction::Char('?')) {
        match app.state {
            // Don't intercept '?' in text input states
            AppState::PasswordEntry => {}
            AppState::BirthdayEditor { .. } => {}
            AppState::Settings if app.settings_state.editing => {}
            // All other states - show help
            _ => {
                app.show_help = true;
                return AppAction::None;
            }
        }
    }

    // Clone state to avoid borrow issues with mutable state fields
    let state = app.state.clone();
    match state {
        AppState::WalletSelection => views::wallet_selector::handle_key(app, action),
        AppState::PasswordEntry => views::unlock::handle_key(app, action),
        AppState::Dashboard { .. } => views::dashboard::handle_key(app, action),
        AppState::AccountDetails { account_id } => {
            views::account_details::handle_key(app, action, account_id)
        }
        AppState::BirthdayEditor { account_id } => {
            views::birthday_editor::handle_key(app, action, account_id)
        }
        AppState::SyncReset => {
            let mut state = std::mem::take(&mut app.sync_reset_state);
            let result = views::sync_reset::handle_key(app, action, &mut state);
            app.sync_reset_state = state;
            result
        }
        AppState::Settings => {
            let mut state = std::mem::take(&mut app.settings_state);
            let result = views::settings::handle_key(app, action, &mut state);
            app.settings_state = state;
            result
        }
        _ => match action {
            KeyAction::Quit | KeyAction::Char('q') => AppAction::Quit,
            KeyAction::Back => {
                app.go_back();
                AppAction::None
            }
            _ => AppAction::None,
        },
    }
}

fn render_placeholder(frame: &mut Frame, area: Rect, state_name: &str) {
    let text = format!("State: {}\n\nPress Esc to go back, q to quit", state_name);
    let block = Block::default()
        .title(" zkore-tui ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));
    let paragraph = Paragraph::new(text)
        .block(block)
        .alignment(Alignment::Center);
    frame.render_widget(paragraph, area);
}

fn default_app_db_path() -> Option<std::path::PathBuf> {
    std::env::var_os("HOME").map(|h| std::path::PathBuf::from(h).join(".zkore").join("app.db"))
}

fn default_wallets_dir() -> Option<std::path::PathBuf> {
    std::env::var_os("HOME").map(|h| std::path::PathBuf::from(h).join(".zkore").join("wallets"))
}
