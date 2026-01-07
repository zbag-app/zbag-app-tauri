//! Help overlay widget showing keybindings.

use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Clear, Paragraph};

use crate::ui::centered_rect;

/// Renders a help overlay showing all keybindings.
pub struct HelpOverlay;

impl HelpOverlay {
    /// Render the help overlay centered on the screen.
    pub fn render(frame: &mut Frame, area: Rect) {
        let dialog_area = centered_rect(60, 70, area);
        frame.render_widget(Clear, dialog_area);

        let lines = vec![
            Line::from(""),
            Line::from(Span::styled(
                "zkore-tui - Keyboard Shortcuts",
                Style::default().add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
            Line::from(""),
            Line::from(Span::styled(
                "Navigation",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from("  j/k or Up/Down    Navigate lists"),
            Line::from("  Tab/Shift+Tab     Switch tabs"),
            Line::from("  Enter             Select/Confirm"),
            Line::from("  Esc               Go back/Cancel"),
            Line::from("  q                 Quit"),
            Line::from(""),
            Line::from(Span::styled(
                "Dashboard",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from("  Accounts tab:"),
            Line::from("    Enter           View account details"),
            Line::from("  Sync tab:"),
            Line::from("    r               Reset sync state"),
            Line::from("  Settings tab:"),
            Line::from("    e               Edit settings"),
            Line::from(""),
            Line::from(Span::styled(
                "Account Details",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from("  e                 Edit birthday height"),
            Line::from(""),
            Line::from(""),
            Line::from(Span::styled(
                "Press any key to close",
                Style::default().fg(Color::DarkGray),
            )),
        ];

        let paragraph = Paragraph::new(lines)
            .block(
                Block::default()
                    .title(" Help ")
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Cyan)),
            )
            .alignment(Alignment::Center);
        frame.render_widget(paragraph, dialog_area);
    }
}
