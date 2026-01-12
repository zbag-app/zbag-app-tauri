//! Wallet selection view.

use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Clear, ListState, Paragraph};

use crate::app::{App, AppAction, AppState};
use crate::event::KeyAction;
use crate::ui::centered_rect;
use crate::ui::widgets::WalletListWidget;

/// Render the wallet selector view
pub fn render(app: &mut App, frame: &mut Frame, area: Rect) {
    let chunks = Layout::vertical([
        Constraint::Length(3), // Header
        Constraint::Min(0),    // Wallet list
        Constraint::Length(3), // Help
    ])
    .split(area);

    // Header
    let header = Paragraph::new("Select a wallet to inspect")
        .block(
            Block::default()
                .title(" zkore-tui ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Cyan)),
        )
        .alignment(Alignment::Center);
    frame.render_widget(header, chunks[0]);

    // Wallet list
    let mut list_state = ListState::default().with_selected(Some(app.selected_wallet_idx));
    WalletListWidget::new(&app.wallets, &mut list_state).render(frame, chunks[1]);
    app.selected_wallet_idx = list_state.selected().unwrap_or(0);

    // Help
    let help = Paragraph::new("j/k: navigate | Enter: select | q: quit")
        .style(Style::default().fg(Color::DarkGray))
        .alignment(Alignment::Center);
    frame.render_widget(help, chunks[2]);

    // Show error if any
    if let Some(ref err) = app.error_message {
        let error_area = centered_rect(60, 20, area);
        let error = Paragraph::new(err.as_str())
            .block(
                Block::default()
                    .title(" Error ")
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Red)),
            )
            .alignment(Alignment::Center)
            .style(Style::default().fg(Color::Red));
        frame.render_widget(Clear, error_area);
        frame.render_widget(error, error_area);
    }
}

/// Handle key input in wallet selector
pub fn handle_key(app: &mut App, action: KeyAction) -> AppAction {
    // Clear error on any key
    if app.error_message.is_some() {
        app.clear_error();
        return AppAction::None;
    }

    match action {
        KeyAction::Quit | KeyAction::Char('q') => AppAction::Quit,
        KeyAction::Up | KeyAction::Char('k') => {
            app.select_prev_wallet();
            AppAction::None
        }
        KeyAction::Down | KeyAction::Char('j') => {
            app.select_next_wallet();
            AppAction::None
        }
        KeyAction::Select => {
            if app.wallets.is_empty() {
                app.set_error("No wallets found");
                AppAction::None
            } else {
                AppAction::ChangeState(AppState::PasswordEntry)
            }
        }
        _ => AppAction::None,
    }
}
