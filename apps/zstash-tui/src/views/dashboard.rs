//! Dashboard view with tabs for Overview, Accounts, Sync, and Settings.

use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Paragraph, Tabs};
use rusqlite::Connection;

use crate::app::{App, AppAction, AppState, DashboardTab};
use crate::data::{self, AccountInfo, SyncState};
use crate::event::KeyAction;

/// Cached dashboard data
pub struct DashboardData {
    pub accounts: Vec<AccountInfo>,
    pub sync_state: SyncState,
    pub selected_account_idx: usize,
}

impl Default for DashboardData {
    fn default() -> Self {
        Self {
            accounts: Vec::new(),
            sync_state: SyncState {
                chain_tip: None,
                fully_scanned_height: None,
                scan_ranges: Vec::new(),
            },
            selected_account_idx: 0,
        }
    }
}

/// Render the dashboard view
pub fn render(app: &App, frame: &mut Frame, area: Rect, tab: DashboardTab, data: &DashboardData) {
    let wallet = match &app.current_wallet {
        Some(w) => w,
        None => return,
    };

    let chunks = Layout::vertical([
        Constraint::Length(3), // Header with wallet name
        Constraint::Length(3), // Tabs
        Constraint::Min(0),    // Content
        Constraint::Length(1), // Help
    ])
    .split(area);

    // Header
    let network_str = match wallet.network {
        zstash_core::domain::Network::Mainnet => "Mainnet",
        zstash_core::domain::Network::Testnet => "Testnet",
    };
    let header = Paragraph::new(format!("{} ({})", wallet.name, network_str))
        .block(
            Block::default()
                .title(" zstash-tui - Dashboard ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Cyan)),
        )
        .alignment(Alignment::Center);
    frame.render_widget(header, chunks[0]);

    // Tabs
    let tab_titles: Vec<&str> = vec!["Overview", "Accounts", "Sync", "Settings"];
    let selected_tab = match tab {
        DashboardTab::Overview => 0,
        DashboardTab::Accounts => 1,
        DashboardTab::Sync => 2,
        DashboardTab::Settings => 3,
    };
    let tabs = Tabs::new(tab_titles)
        .select(selected_tab)
        .style(Style::default().fg(Color::White))
        .highlight_style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )
        .divider(" | ");
    frame.render_widget(tabs, chunks[1]);

    // Content based on tab
    match tab {
        DashboardTab::Overview => render_overview(wallet, frame, chunks[2], data),
        DashboardTab::Accounts => render_accounts(frame, chunks[2], data),
        DashboardTab::Sync => render_sync(frame, chunks[2], data),
        DashboardTab::Settings => render_settings(wallet, frame, chunks[2]),
    }

    // Help
    let help_text = match tab {
        DashboardTab::Accounts => {
            "j/k: navigate | Enter: view account | Tab: switch tabs | Esc: back | q: quit"
        }
        DashboardTab::Sync => "r: reset sync | Tab: switch tabs | Esc: back | q: quit",
        DashboardTab::Settings => "e: edit settings | Tab: switch tabs | Esc: back | q: quit",
        _ => "Tab: switch tabs | Esc: back to wallet list | q: quit",
    };
    let help = Paragraph::new(help_text)
        .style(Style::default().fg(Color::DarkGray))
        .alignment(Alignment::Center);
    frame.render_widget(help, chunks[3]);
}

fn render_overview(
    wallet: &zstash_core::domain::WalletInfo,
    frame: &mut Frame,
    area: Rect,
    data: &DashboardData,
) {
    let sync_status = if let Some(tip) = data.sync_state.chain_tip {
        if let Some(scanned) = data.sync_state.fully_scanned_height {
            if scanned >= tip {
                format!("Synced to height {}", tip)
            } else {
                format!("Syncing: {} / {}", scanned, tip)
            }
        } else {
            format!("Chain tip: {}", tip)
        }
    } else {
        "Not synced".to_string()
    };

    let content = vec![
        Line::from(""),
        Line::from(vec![
            Span::styled("Accounts: ", Style::default().fg(Color::Gray)),
            Span::styled(
                format!("{}", data.accounts.len()),
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("Sync Status: ", Style::default().fg(Color::Gray)),
            Span::styled(&sync_status, Style::default().fg(Color::White)),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("Wallet Type: ", Style::default().fg(Color::Gray)),
            Span::styled(
                format!("{:?}", wallet.wallet_type),
                Style::default().fg(Color::White),
            ),
        ]),
    ];

    let paragraph = Paragraph::new(content).block(
        Block::default()
            .title(" Overview ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::DarkGray)),
    );
    frame.render_widget(paragraph, area);
}

fn render_accounts(frame: &mut Frame, area: Rect, data: &DashboardData) {
    if data.accounts.is_empty() {
        let content = Paragraph::new("No accounts found")
            .block(
                Block::default()
                    .title(" Accounts ")
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::DarkGray)),
            )
            .alignment(Alignment::Center);
        frame.render_widget(content, area);
        return;
    }

    let rows: Vec<Line> = data
        .accounts
        .iter()
        .enumerate()
        .map(|(i, account)| {
            let prefix = if i == data.selected_account_idx {
                "> "
            } else {
                "  "
            };
            let style = if i == data.selected_account_idx {
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };
            Line::styled(
                format!(
                    "{}{}: {} (birthday: {})",
                    prefix, account.id, account.name, account.birthday_height
                ),
                style,
            )
        })
        .collect();

    let content = Paragraph::new(rows).block(
        Block::default()
            .title(" Accounts ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::DarkGray)),
    );
    frame.render_widget(content, area);
}

fn render_sync(frame: &mut Frame, area: Rect, data: &DashboardData) {
    let mut lines = vec![Line::from("")];

    // Chain tip
    if let Some(tip) = data.sync_state.chain_tip {
        lines.push(Line::from(vec![
            Span::styled("Chain Tip: ", Style::default().fg(Color::Gray)),
            Span::styled(format!("{}", tip), Style::default().fg(Color::White)),
        ]));
    } else {
        lines.push(Line::from(vec![
            Span::styled("Chain Tip: ", Style::default().fg(Color::Gray)),
            Span::styled("Unknown", Style::default().fg(Color::Yellow)),
        ]));
    }

    // Fully scanned height
    lines.push(Line::from(vec![
        Span::styled("Scanned Height: ", Style::default().fg(Color::Gray)),
        Span::styled(
            data.sync_state
                .fully_scanned_height
                .map(|h| h.to_string())
                .unwrap_or_else(|| "None".to_string()),
            Style::default().fg(Color::White),
        ),
    ]));

    lines.push(Line::from(""));

    // Pending scan ranges
    if data.sync_state.scan_ranges.is_empty() {
        lines.push(Line::from(Span::styled(
            "No pending scan ranges",
            Style::default().fg(Color::Green),
        )));
    } else {
        lines.push(Line::from(Span::styled(
            format!("Pending scan ranges: {}", data.sync_state.scan_ranges.len()),
            Style::default().fg(Color::Yellow),
        )));
        for range in data.sync_state.scan_ranges.iter().take(5) {
            lines.push(Line::from(format!(
                "  {} - {} (priority: {})",
                range.start, range.end, range.priority
            )));
        }
        if data.sync_state.scan_ranges.len() > 5 {
            lines.push(Line::from(format!(
                "  ... and {} more",
                data.sync_state.scan_ranges.len() - 5
            )));
        }
    }

    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "Press 'r' to reset sync state",
        Style::default().fg(Color::Cyan),
    )));

    let content = Paragraph::new(lines).block(
        Block::default()
            .title(" Sync Status ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::DarkGray)),
    );
    frame.render_widget(content, area);
}

fn render_settings(wallet: &zstash_core::domain::WalletInfo, frame: &mut Frame, area: Rect) {
    let remember_unlock = if wallet.remember_unlock_enabled {
        "Enabled"
    } else {
        "Disabled"
    };

    let content = vec![
        Line::from(""),
        Line::from(vec![
            Span::styled("Wallet Name: ", Style::default().fg(Color::Gray)),
            Span::styled(&wallet.name, Style::default().fg(Color::White)),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("Remember Unlock: ", Style::default().fg(Color::Gray)),
            Span::styled(remember_unlock, Style::default().fg(Color::White)),
        ]),
        Line::from(""),
        Line::from(Span::styled(
            "Press 'e' to edit settings",
            Style::default().fg(Color::Cyan),
        )),
    ];

    let paragraph = Paragraph::new(content).block(
        Block::default()
            .title(" Settings ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::DarkGray)),
    );
    frame.render_widget(paragraph, area);
}

/// Handle key input in dashboard
pub fn handle_key(app: &mut App, action: KeyAction) -> AppAction {
    let tab = match app.state {
        AppState::Dashboard { tab } => tab,
        _ => return AppAction::None,
    };

    match action {
        KeyAction::Quit | KeyAction::Char('q') => AppAction::Quit,
        KeyAction::Back => {
            app.current_wallet = None;
            app.cached_dek_bytes = None; // Clear cached DEK when going back
            AppAction::ChangeState(AppState::WalletSelection)
        }
        KeyAction::NextTab => {
            app.next_tab();
            AppAction::None
        }
        KeyAction::PrevTab => {
            app.prev_tab();
            AppAction::None
        }
        // j/k navigation for Accounts tab
        KeyAction::Up | KeyAction::Char('k') if tab == DashboardTab::Accounts => {
            let data = &mut app.dashboard_data;
            if !data.accounts.is_empty() && data.selected_account_idx > 0 {
                data.selected_account_idx -= 1;
            }
            AppAction::None
        }
        KeyAction::Down | KeyAction::Char('j') if tab == DashboardTab::Accounts => {
            let data = &mut app.dashboard_data;
            if !data.accounts.is_empty() && data.selected_account_idx < data.accounts.len() - 1 {
                data.selected_account_idx += 1;
            }
            AppAction::None
        }
        KeyAction::Select if tab == DashboardTab::Accounts => {
            let data = &app.dashboard_data;
            if let Some(account) = data.accounts.get(data.selected_account_idx) {
                AppAction::ChangeState(AppState::AccountDetails {
                    account_id: account.id,
                })
            } else {
                AppAction::None
            }
        }
        // Reset sync state from Sync tab
        KeyAction::Char('r') if tab == DashboardTab::Sync => {
            app.sync_reset_state.reset();
            AppAction::ChangeState(AppState::SyncReset)
        }
        // Edit settings from Settings tab
        KeyAction::Char('e') if tab == DashboardTab::Settings => {
            app.settings_state.reset();
            AppAction::ChangeState(AppState::Settings)
        }
        _ => AppAction::None,
    }
}

/// Load dashboard data from the wallet database
pub fn load_data(
    wallet_db: &Connection,
    network: zstash_core::domain::Network,
) -> anyhow::Result<DashboardData> {
    let accounts = data::get_accounts(wallet_db, network)?;
    let sync_state = data::get_sync_state(wallet_db)?;

    Ok(DashboardData {
        accounts,
        sync_state,
        selected_account_idx: 0,
    })
}
