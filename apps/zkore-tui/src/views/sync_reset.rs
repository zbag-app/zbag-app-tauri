//! Sync reset view.

use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Clear, Paragraph};

use crate::app::{App, AppAction, AppState, DashboardTab};
use crate::data;
use crate::event::KeyAction;
use crate::ui::centered_rect;
use zkore_engine::encryption::Dek;

/// State for sync reset
#[derive(Default)]
pub struct SyncResetState {
    pub show_confirmation: bool,
    pub confirm_selected: bool,
    pub error: Option<String>,
}

impl SyncResetState {
    /// Reset the state
    pub fn reset(&mut self) {
        self.show_confirmation = false;
        self.confirm_selected = false;
        self.error = None;
    }
}

/// Render the sync reset view
pub fn render(_app: &App, frame: &mut Frame, area: Rect, state: &SyncResetState) {
    let chunks = Layout::vertical([
        Constraint::Length(3), // Header
        Constraint::Min(0),    // Content
        Constraint::Length(1), // Help
    ])
    .split(area);

    // Header
    let header = Paragraph::new("Reset Sync State")
        .block(
            Block::default()
                .title(" Sync Reset ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Yellow)),
        )
        .alignment(Alignment::Center);
    frame.render_widget(header, chunks[0]);

    // Content
    let mut lines = vec![
        Line::from(""),
        Line::from(Span::styled(
            "This will clear all sync progress and scan ranges.",
            Style::default().fg(Color::White),
        )),
        Line::from(""),
        Line::from(Span::styled(
            "The wallet will need to rescan from the account birthday",
            Style::default().fg(Color::White),
        )),
        Line::from(Span::styled(
            "on the next sync operation.",
            Style::default().fg(Color::White),
        )),
        Line::from(""),
        Line::from(Span::styled(
            "WARNING: This operation cannot be undone.",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
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
        "Press Enter to proceed, Esc to cancel",
        Style::default().fg(Color::Cyan),
    )));

    let paragraph = Paragraph::new(lines).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::DarkGray)),
    );
    frame.render_widget(paragraph, chunks[1]);

    // Help
    let help = Paragraph::new("Enter: reset | Esc: cancel | q: quit")
        .style(Style::default().fg(Color::DarkGray))
        .alignment(Alignment::Center);
    frame.render_widget(help, chunks[2]);

    // Confirmation dialog
    if state.show_confirmation {
        render_confirmation(frame, area, state.confirm_selected);
    }
}

fn render_confirmation(frame: &mut Frame, area: Rect, confirm_selected: bool) {
    let dialog_area = centered_rect(50, 35, area);
    frame.render_widget(Clear, dialog_area);

    let lines = vec![
        Line::from(""),
        Line::from(Span::styled(
            "Confirm Sync Reset",
            Style::default().add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from("Are you sure you want to reset sync state?"),
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
pub fn handle_key(app: &mut App, action: KeyAction, state: &mut SyncResetState) -> AppAction {
    if state.show_confirmation {
        return handle_confirmation_key(app, action, state);
    }

    match action {
        KeyAction::Quit | KeyAction::Char('q') => AppAction::Quit,
        KeyAction::Back => {
            state.error = None;
            AppAction::ChangeState(AppState::Dashboard {
                tab: DashboardTab::Sync,
            })
        }
        KeyAction::Select => {
            state.show_confirmation = true;
            state.confirm_selected = false;
            AppAction::None
        }
        _ => AppAction::None,
    }
}

fn handle_confirmation_key(
    app: &mut App,
    action: KeyAction,
    state: &mut SyncResetState,
) -> AppAction {
    match action {
        KeyAction::Back => {
            state.show_confirmation = false;
            AppAction::None
        }
        KeyAction::NextTab | KeyAction::PrevTab => {
            state.confirm_selected = !state.confirm_selected;
            AppAction::None
        }
        KeyAction::Select => {
            if state.confirm_selected {
                match perform_sync_reset(app) {
                    Ok(()) => {
                        state.show_confirmation = false;
                        state.error = None;
                        AppAction::ChangeState(AppState::Dashboard {
                            tab: DashboardTab::Sync,
                        })
                    }
                    Err(e) => {
                        state.show_confirmation = false;
                        state.error = Some(format!("Failed: {}", e));
                        AppAction::None
                    }
                }
            } else {
                state.show_confirmation = false;
                AppAction::None
            }
        }
        _ => AppAction::None,
    }
}

fn perform_sync_reset(app: &mut App) -> anyhow::Result<()> {
    let wallet = app
        .current_wallet
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("No wallet selected"))?;

    let dek_bytes = app
        .cached_dek_bytes
        .ok_or_else(|| anyhow::anyhow!("Session expired - please unlock wallet again"))?;
    let dek = Dek(dek_bytes);

    let db_path = data::wallet_db_path(&app.config.wallets_dir, wallet.network, wallet.id);
    let wallet_db = data::open_wallet_db(&db_path, &dek)?;

    data::reset_sync_state(&wallet_db)?;

    // Reload dashboard data
    if let Ok(new_data) = crate::views::dashboard::load_data(&wallet_db, wallet.network) {
        app.dashboard_data = new_data;
    }

    Ok(())
}
