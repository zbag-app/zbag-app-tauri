//! Wallet list widget.

use ratatui::{
    prelude::*,
    widgets::{Block, Borders, List, ListItem, ListState},
};
use zstash_core::domain::{Network, WalletInfo};

pub struct WalletListWidget<'a> {
    wallets: &'a [WalletInfo],
    state: &'a mut ListState,
}

impl<'a> WalletListWidget<'a> {
    pub fn new(wallets: &'a [WalletInfo], state: &'a mut ListState) -> Self {
        Self { wallets, state }
    }

    pub fn render(self, frame: &mut Frame, area: Rect) {
        let items: Vec<ListItem> = self
            .wallets
            .iter()
            .map(|wallet| {
                let network_badge = match wallet.network {
                    Network::Mainnet => "[M]",
                    Network::Testnet => "[T]",
                };
                let line = Line::from(vec![
                    Span::styled(
                        format!("{} ", network_badge),
                        Style::default().fg(match wallet.network {
                            Network::Mainnet => Color::Green,
                            Network::Testnet => Color::Yellow,
                        }),
                    ),
                    Span::raw(&wallet.name),
                ]);
                ListItem::new(line)
            })
            .collect();

        let list = List::new(items)
            .block(
                Block::default()
                    .title(" Wallets ")
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Cyan)),
            )
            .highlight_style(
                Style::default()
                    .bg(Color::DarkGray)
                    .add_modifier(Modifier::BOLD),
            )
            .highlight_symbol("> ");

        frame.render_stateful_widget(list, area, self.state);
    }
}
