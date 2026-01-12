//! Settings view for editing wallet settings.

use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Paragraph};

use crate::app::{App, AppAction, AppState, DashboardTab};
use crate::data;
use crate::event::KeyAction;

/// Settings option
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingsOption {
    WalletName,
    RememberUnlock,
}

/// State for settings view
#[derive(Default)]
pub struct SettingsState {
    pub selected: usize,
    pub editing: bool,
    pub edit_buffer: String,
    pub error: Option<String>,
}

impl SettingsState {
    /// Reset the state
    pub fn reset(&mut self) {
        self.selected = 0;
        self.editing = false;
        self.edit_buffer.clear();
        self.error = None;
    }
}

const OPTIONS: &[SettingsOption] = &[SettingsOption::WalletName, SettingsOption::RememberUnlock];

/// Render the settings view
pub fn render(app: &App, frame: &mut Frame, area: Rect, state: &SettingsState) {
    let wallet = match &app.current_wallet {
        Some(w) => w,
        None => return,
    };

    let chunks = Layout::vertical([
        Constraint::Length(3), // Header
        Constraint::Min(0),    // Content
        Constraint::Length(1), // Help
    ])
    .split(area);

    // Header
    let header = Paragraph::new("Edit wallet settings")
        .block(
            Block::default()
                .title(" Settings ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Cyan)),
        )
        .alignment(Alignment::Center);
    frame.render_widget(header, chunks[0]);

    // Content
    let mut lines = vec![Line::from("")];

    for (i, option) in OPTIONS.iter().enumerate() {
        let is_selected = i == state.selected;
        let prefix = if is_selected { "> " } else { "  " };

        let (label, value) = match option {
            SettingsOption::WalletName => {
                let value = if state.editing && is_selected {
                    format!("{}|", state.edit_buffer)
                } else {
                    wallet.name.clone()
                };
                ("Wallet Name", value)
            }
            SettingsOption::RememberUnlock => {
                let value = if wallet.remember_unlock_enabled {
                    "Enabled".to_string()
                } else {
                    "Disabled".to_string()
                };
                ("Remember Unlock", value)
            }
        };

        let style = if is_selected {
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default()
        };

        lines.push(Line::from(vec![
            Span::styled(prefix, style),
            Span::styled(format!("{}: ", label), Style::default().fg(Color::Gray)),
            Span::styled(value, style),
        ]));
        lines.push(Line::from(""));
    }

    if let Some(ref err) = state.error {
        lines.push(Line::from(Span::styled(
            err.as_str(),
            Style::default().fg(Color::Red),
        )));
        lines.push(Line::from(""));
    }

    let paragraph = Paragraph::new(lines).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::DarkGray)),
    );
    frame.render_widget(paragraph, chunks[1]);

    // Help
    let help_text = if state.editing {
        "Enter: save | Esc: cancel"
    } else {
        "j/k: navigate | Enter: edit/toggle | Esc: back | q: quit"
    };
    let help = Paragraph::new(help_text)
        .style(Style::default().fg(Color::DarkGray))
        .alignment(Alignment::Center);
    frame.render_widget(help, chunks[2]);
}

/// Handle key input
pub fn handle_key(app: &mut App, action: KeyAction, state: &mut SettingsState) -> AppAction {
    if state.editing {
        return handle_editing_key(app, action, state);
    }

    match action {
        KeyAction::Quit | KeyAction::Char('q') => AppAction::Quit,
        KeyAction::Back => {
            state.error = None;
            AppAction::ChangeState(AppState::Dashboard {
                tab: DashboardTab::Settings,
            })
        }
        KeyAction::Up | KeyAction::Char('k') => {
            if state.selected > 0 {
                state.selected -= 1;
            }
            AppAction::None
        }
        KeyAction::Down | KeyAction::Char('j') => {
            if state.selected < OPTIONS.len() - 1 {
                state.selected += 1;
            }
            AppAction::None
        }
        KeyAction::Select => {
            match OPTIONS.get(state.selected) {
                Some(SettingsOption::WalletName) => {
                    // Start editing wallet name
                    if let Some(wallet) = &app.current_wallet {
                        state.edit_buffer = wallet.name.clone();
                    }
                    state.editing = true;
                }
                Some(SettingsOption::RememberUnlock) => {
                    // Toggle remember unlock
                    toggle_remember_unlock(app, state);
                }
                None => {}
            }
            AppAction::None
        }
        _ => AppAction::None,
    }
}

fn handle_editing_key(app: &mut App, action: KeyAction, state: &mut SettingsState) -> AppAction {
    match action {
        KeyAction::Back => {
            state.editing = false;
            state.edit_buffer.clear();
            AppAction::None
        }
        KeyAction::Char(c) => {
            state.edit_buffer.push(c);
            AppAction::None
        }
        KeyAction::Backspace => {
            state.edit_buffer.pop();
            AppAction::None
        }
        KeyAction::Select => {
            // Save the changes
            if let Some(SettingsOption::WalletName) = OPTIONS.get(state.selected) {
                if let Err(e) = save_wallet_name(app, &state.edit_buffer) {
                    state.error = Some(format!("Failed to save: {}", e));
                } else {
                    // Update the wallet name in app state
                    if let Some(wallet) = &mut app.current_wallet {
                        wallet.name.clone_from(&state.edit_buffer);
                    }
                }
            }
            state.editing = false;
            state.edit_buffer.clear();
            AppAction::None
        }
        _ => AppAction::None,
    }
}

fn save_wallet_name(app: &App, new_name: &str) -> anyhow::Result<()> {
    let wallet = app
        .current_wallet
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("No wallet selected"))?;

    if new_name.trim().is_empty() {
        anyhow::bail!("Name cannot be empty");
    }

    let app_db = data::open_app_db(&app.config.app_db_path)?;
    data::update_wallet_name(&app_db, wallet.id, new_name)?;

    Ok(())
}

fn toggle_remember_unlock(app: &mut App, state: &mut SettingsState) {
    let Some(wallet) = &app.current_wallet else {
        return;
    };

    let new_value = !wallet.remember_unlock_enabled;

    match (|| -> anyhow::Result<()> {
        let app_db = data::open_app_db(&app.config.app_db_path)?;
        data::set_remember_unlock(&app_db, wallet.id, new_value)?;
        Ok(())
    })() {
        Ok(()) => {
            if let Some(w) = &mut app.current_wallet {
                w.remember_unlock_enabled = new_value;
            }
            state.error = None;
        }
        Err(e) => {
            state.error = Some(format!("Failed to toggle: {}", e));
        }
    }
}
