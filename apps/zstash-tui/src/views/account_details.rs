//! Account details view.

use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Paragraph};

use crate::app::{App, AppAction, AppState, DashboardTab};
use crate::event::KeyAction;

/// Render the account details view
pub fn render(app: &App, frame: &mut Frame, area: Rect, account_id: u32) {
    let account = app
        .dashboard_data
        .accounts
        .iter()
        .find(|a| a.id == account_id);

    let chunks = Layout::vertical([
        Constraint::Length(3), // Header
        Constraint::Min(0),    // Content
        Constraint::Length(1), // Help
    ])
    .split(area);

    // Header
    let title = match account {
        Some(a) => format!(" Account: {} ", a.name),
        None => " Account Details ".to_string(),
    };
    let header = Paragraph::new(
        app.current_wallet
            .as_ref()
            .map(|w| w.name.as_str())
            .unwrap_or(""),
    )
    .block(
        Block::default()
            .title(title)
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan)),
    )
    .alignment(Alignment::Center);
    frame.render_widget(header, chunks[0]);

    // Content
    let content = match account {
        Some(a) => {
            vec![
                Line::from(""),
                Line::from(vec![
                    Span::styled("Account ID: ", Style::default().fg(Color::Gray)),
                    Span::styled(format!("{}", a.id), Style::default().fg(Color::White)),
                ]),
                Line::from(""),
                Line::from(vec![
                    Span::styled("Account UUID: ", Style::default().fg(Color::Gray)),
                    Span::styled(&a.uuid, Style::default().fg(Color::White)),
                ]),
                Line::from(""),
                Line::from(vec![
                    Span::styled("Birthday Height: ", Style::default().fg(Color::Yellow)),
                    Span::styled(
                        format!("{}", a.birthday_height),
                        Style::default()
                            .fg(Color::White)
                            .add_modifier(Modifier::BOLD),
                    ),
                ]),
                Line::from(""),
                Line::from(Span::styled(
                    "Press 'e' to edit birthday height",
                    Style::default().fg(Color::Cyan),
                )),
            ]
        }
        None => {
            vec![
                Line::from(""),
                Line::from(Span::styled(
                    "Account not found",
                    Style::default().fg(Color::Red),
                )),
            ]
        }
    };

    let paragraph = Paragraph::new(content).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::DarkGray)),
    );
    frame.render_widget(paragraph, chunks[1]);

    // Help
    let help = Paragraph::new("e: edit birthday | Esc: back | q: quit")
        .style(Style::default().fg(Color::DarkGray))
        .alignment(Alignment::Center);
    frame.render_widget(help, chunks[2]);
}

/// Handle key input
pub fn handle_key(_app: &mut App, action: KeyAction, account_id: u32) -> AppAction {
    match action {
        KeyAction::Quit | KeyAction::Char('q') => AppAction::Quit,
        KeyAction::Back => AppAction::ChangeState(AppState::Dashboard {
            tab: DashboardTab::Accounts,
        }),
        KeyAction::Char('e') => AppAction::ChangeState(AppState::BirthdayEditor { account_id }),
        _ => AppAction::None,
    }
}
