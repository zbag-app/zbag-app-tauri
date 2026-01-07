//! Confirmation dialog widget.

use ratatui::{
    prelude::*,
    widgets::{Block, Borders, Clear, Paragraph},
};

use crate::ui::layout::centered_rect;

pub struct ConfirmationDialog<'a> {
    title: &'a str,
    message: &'a str,
    warning: Option<&'a str>,
    confirm_selected: bool,
}

impl<'a> ConfirmationDialog<'a> {
    pub fn new(title: &'a str, message: &'a str) -> Self {
        Self {
            title,
            message,
            warning: None,
            confirm_selected: false,
        }
    }

    pub fn warning(mut self, warning: &'a str) -> Self {
        self.warning = Some(warning);
        self
    }

    pub fn confirm_selected(mut self, selected: bool) -> Self {
        self.confirm_selected = selected;
        self
    }

    pub fn render(self, frame: &mut Frame, area: Rect) {
        let dialog_area = centered_rect(50, 40, area);

        // Clear the background
        frame.render_widget(Clear, dialog_area);

        let mut lines = vec![Line::from(""), Line::from(self.message), Line::from("")];

        if let Some(warning) = self.warning {
            lines.push(Line::from(Span::styled(
                warning,
                Style::default().fg(Color::Yellow),
            )));
            lines.push(Line::from(""));
        }

        // Buttons
        let yes_style = if self.confirm_selected {
            Style::default()
                .bg(Color::Green)
                .fg(Color::Black)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::Green)
        };
        let no_style = if !self.confirm_selected {
            Style::default()
                .bg(Color::Red)
                .fg(Color::Black)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::Red)
        };

        lines.push(Line::from(vec![
            Span::raw("  "),
            Span::styled(" Yes ", yes_style),
            Span::raw("    "),
            Span::styled(" No ", no_style),
            Span::raw("  "),
        ]));
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "Tab to switch, Enter to confirm",
            Style::default().fg(Color::DarkGray),
        )));

        let paragraph = Paragraph::new(lines)
            .block(
                Block::default()
                    .title(self.title)
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Yellow)),
            )
            .alignment(Alignment::Center);

        frame.render_widget(paragraph, dialog_area);
    }
}
