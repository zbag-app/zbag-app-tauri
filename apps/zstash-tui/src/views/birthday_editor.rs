//! Birthday height editor view.

use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Clear, Paragraph};

use crate::app::{App, AppAction, AppState, DashboardTab};
use crate::data;
use crate::event::KeyAction;
use crate::ui::centered_rect;

/// State for the birthday editor
#[derive(Default)]
pub struct BirthdayEditorState {
    pub input: String,
    pub error: Option<String>,
    pub show_confirmation: bool,
    /// true = Yes selected, false = No selected
    pub confirm_selected: bool,
}

impl BirthdayEditorState {
    /// Reset the editor state
    pub fn reset(&mut self) {
        self.input.clear();
        self.error = None;
        self.show_confirmation = false;
        self.confirm_selected = false;
    }
}

/// Minimum birthday heights (Sapling activation)
pub const MAINNET_MIN_BIRTHDAY: u32 = 419200;
pub const TESTNET_MIN_BIRTHDAY: u32 = 280000;

/// Render the birthday editor view
pub fn render(app: &App, frame: &mut Frame, area: Rect, account_id: u32) {
    let account = app
        .dashboard_data
        .accounts
        .iter()
        .find(|a| a.id == account_id);
    let network = app.current_wallet.as_ref().map(|w| w.network);
    let state = &app.birthday_editor_state;

    let min_height = match network {
        Some(zstash_core::domain::Network::Mainnet) => MAINNET_MIN_BIRTHDAY,
        Some(zstash_core::domain::Network::Testnet) => TESTNET_MIN_BIRTHDAY,
        None => 0,
    };

    let chunks = Layout::vertical([
        Constraint::Length(3), // Header
        Constraint::Min(0),    // Content
        Constraint::Length(1), // Help
    ])
    .split(area);

    // Header
    let title = match account {
        Some(a) => format!(" Edit Birthday: {} ", a.name),
        None => " Edit Birthday ".to_string(),
    };
    let header = Paragraph::new("Modify the wallet birthday height")
        .block(
            Block::default()
                .title(title)
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Yellow)),
        )
        .alignment(Alignment::Center);
    frame.render_widget(header, chunks[0]);

    // Content
    let current_height = account.map(|a| a.birthday_height).unwrap_or(0);

    let mut lines = vec![
        Line::from(""),
        Line::from(vec![
            Span::styled("Current Birthday: ", Style::default().fg(Color::Gray)),
            Span::styled(
                format!("{}", current_height),
                Style::default().fg(Color::White),
            ),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("New Birthday: ", Style::default().fg(Color::Cyan)),
            Span::styled(
                &state.input,
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                "_",
                Style::default()
                    .fg(Color::Gray)
                    .add_modifier(Modifier::SLOW_BLINK),
            ),
        ]),
        Line::from(""),
        Line::from(Span::styled(
            format!("(minimum: {})", min_height),
            Style::default().fg(Color::DarkGray),
        )),
    ];

    if let Some(ref err) = state.error {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            err.as_str(),
            Style::default().fg(Color::Red),
        )));
    }

    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "WARNING: Changing the birthday will clear sync state",
        Style::default().fg(Color::Yellow),
    )));
    lines.push(Line::from(Span::styled(
        "and require a full rescan from the new height.",
        Style::default().fg(Color::Yellow),
    )));

    let paragraph = Paragraph::new(lines).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::DarkGray)),
    );
    frame.render_widget(paragraph, chunks[1]);

    // Help
    let help = Paragraph::new("Enter: confirm | Esc: cancel | 0-9: enter height")
        .style(Style::default().fg(Color::DarkGray))
        .alignment(Alignment::Center);
    frame.render_widget(help, chunks[2]);

    // Confirmation dialog overlay
    if state.show_confirmation {
        render_confirmation(frame, area, &state.input, state.confirm_selected);
    }
}

fn render_confirmation(frame: &mut Frame, area: Rect, new_height: &str, confirm_selected: bool) {
    let dialog_area = centered_rect(60, 40, area);
    frame.render_widget(Clear, dialog_area);

    let lines = vec![
        Line::from(""),
        Line::from(Span::styled(
            "Confirm Birthday Change",
            Style::default().add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(format!("New birthday height: {}", new_height)),
        Line::from(""),
        Line::from(Span::styled(
            "This will clear all sync progress and require",
            Style::default().fg(Color::Yellow),
        )),
        Line::from(Span::styled(
            "a full rescan from the new birthday.",
            Style::default().fg(Color::Yellow),
        )),
        Line::from(""),
        Line::from(vec![
            Span::raw("  "),
            if confirm_selected {
                Span::styled(
                    " Yes ",
                    Style::default()
                        .bg(Color::Green)
                        .fg(Color::Black)
                        .add_modifier(Modifier::BOLD),
                )
            } else {
                Span::styled(" Yes ", Style::default().fg(Color::Green))
            },
            Span::raw("    "),
            if !confirm_selected {
                Span::styled(
                    " No ",
                    Style::default()
                        .bg(Color::Red)
                        .fg(Color::Black)
                        .add_modifier(Modifier::BOLD),
                )
            } else {
                Span::styled(" No ", Style::default().fg(Color::Red))
            },
        ]),
        Line::from(""),
        Line::from(Span::styled(
            "Tab to switch, Enter to confirm",
            Style::default().fg(Color::DarkGray),
        )),
    ];

    let paragraph = Paragraph::new(lines)
        .block(
            Block::default()
                .title(" Confirm ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Yellow)),
        )
        .alignment(Alignment::Center);
    frame.render_widget(paragraph, dialog_area);
}

/// Handle key input
pub fn handle_key(app: &mut App, action: KeyAction, account_id: u32) -> AppAction {
    // Handle confirmation dialog
    if app.birthday_editor_state.show_confirmation {
        return handle_confirmation_key(app, action, account_id);
    }

    match action {
        KeyAction::Quit => AppAction::Quit,
        KeyAction::Char('q') => AppAction::Quit,
        KeyAction::Back => {
            app.birthday_editor_state.reset();
            AppAction::ChangeState(AppState::AccountDetails { account_id })
        }
        KeyAction::Char(c) if c.is_ascii_digit() => {
            app.birthday_editor_state.input.push(c);
            app.birthday_editor_state.error = None;
            AppAction::None
        }
        KeyAction::Backspace => {
            app.birthday_editor_state.input.pop();
            app.birthday_editor_state.error = None;
            AppAction::None
        }
        KeyAction::Select => {
            // Validate and show confirmation
            let input = app.birthday_editor_state.input.clone();
            if let Err(e) = validate_birthday(app, &input) {
                app.birthday_editor_state.error = Some(e);
                AppAction::None
            } else {
                app.birthday_editor_state.show_confirmation = true;
                app.birthday_editor_state.confirm_selected = false; // Default to No
                AppAction::None
            }
        }
        _ => AppAction::None,
    }
}

fn handle_confirmation_key(app: &mut App, action: KeyAction, account_id: u32) -> AppAction {
    match action {
        KeyAction::Back => {
            app.birthday_editor_state.show_confirmation = false;
            AppAction::None
        }
        KeyAction::NextTab | KeyAction::PrevTab => {
            app.birthday_editor_state.confirm_selected =
                !app.birthday_editor_state.confirm_selected;
            AppAction::None
        }
        KeyAction::Select => {
            if app.birthday_editor_state.confirm_selected {
                // User selected Yes - perform the update
                let input = app.birthday_editor_state.input.clone();
                match perform_birthday_update(app, account_id, &input) {
                    Ok(()) => {
                        app.birthday_editor_state.reset();
                        // Return to dashboard accounts tab
                        AppAction::ChangeState(AppState::Dashboard {
                            tab: DashboardTab::Accounts,
                        })
                    }
                    Err(e) => {
                        app.birthday_editor_state.show_confirmation = false;
                        app.birthday_editor_state.error = Some(format!("Failed to update: {}", e));
                        AppAction::None
                    }
                }
            } else {
                // User selected No - go back to editing
                app.birthday_editor_state.show_confirmation = false;
                AppAction::None
            }
        }
        _ => AppAction::None,
    }
}

fn validate_birthday(app: &App, input: &str) -> Result<u32, String> {
    if input.is_empty() {
        return Err("Please enter a height".to_string());
    }

    let height: u32 = input.parse().map_err(|_| "Invalid number".to_string())?;

    let network = app.current_wallet.as_ref().map(|w| w.network);
    let min_height = match network {
        Some(zstash_core::domain::Network::Mainnet) => MAINNET_MIN_BIRTHDAY,
        Some(zstash_core::domain::Network::Testnet) => TESTNET_MIN_BIRTHDAY,
        None => return Err("Unknown network".to_string()),
    };

    if height < min_height {
        return Err(format!(
            "Height must be >= {} (Sapling activation)",
            min_height
        ));
    }

    Ok(height)
}

fn perform_birthday_update(
    app: &mut App,
    account_id: u32,
    new_height_str: &str,
) -> anyhow::Result<()> {
    let wallet = app
        .current_wallet
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("No wallet selected"))?;

    let account = app
        .dashboard_data
        .accounts
        .iter()
        .find(|a| a.id == account_id)
        .ok_or_else(|| anyhow::anyhow!("Account not found"))?;

    let new_height: u32 = new_height_str.parse()?;

    // Use cached DEK bytes to open the wallet database
    let dek_bytes = app
        .cached_dek_bytes
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("Session expired - please unlock wallet again"))?;

    // Reconstruct the DEK from cached bytes
    let dek = zstash_engine::encryption::Dek(*dek_bytes);

    // Open wallet database
    let db_path = data::wallet_db_path(&app.config.wallets_dir, wallet.network, wallet.id);
    let wallet_db = data::open_wallet_db(&db_path, &dek)?;

    // Update birthday height
    data::update_birthday_height(&wallet_db, &account.uuid, new_height)?;

    // Reload dashboard data
    if let Ok(new_data) = crate::views::dashboard::load_data(&wallet_db, wallet.network) {
        app.dashboard_data = new_data;
    }

    Ok(())
}
