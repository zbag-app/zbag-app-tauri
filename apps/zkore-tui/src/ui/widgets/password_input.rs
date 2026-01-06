//! Masked password input widget.

use ratatui::{
    prelude::*,
    widgets::{Block, Borders, Paragraph},
};

pub struct PasswordInput<'a> {
    input: &'a str,
    title: &'a str,
    error: Option<&'a str>,
}

impl<'a> PasswordInput<'a> {
    pub fn new(input: &'a str) -> Self {
        Self {
            input,
            title: " Enter Password ",
            error: None,
        }
    }

    pub fn title(mut self, title: &'a str) -> Self {
        self.title = title;
        self
    }

    pub fn error(mut self, error: Option<&'a str>) -> Self {
        self.error = error;
        self
    }

    pub fn render(self, frame: &mut Frame, area: Rect) {
        // Create masked password display
        let masked: String = "*".repeat(self.input.len());

        let mut lines = vec![
            Line::from(""),
            Line::from(vec![
                Span::raw("Password: "),
                Span::styled(&masked, Style::default().fg(Color::White)),
                Span::styled(
                    "_",
                    Style::default()
                        .fg(Color::Gray)
                        .add_modifier(Modifier::SLOW_BLINK),
                ),
            ]),
            Line::from(""),
        ];

        if let Some(err) = self.error {
            lines.push(Line::from(Span::styled(
                err,
                Style::default().fg(Color::Red),
            )));
            lines.push(Line::from(""));
        }

        lines.push(Line::from(Span::styled(
            "Press Enter to unlock, Esc to cancel",
            Style::default().fg(Color::DarkGray),
        )));

        let border_color = if self.error.is_some() {
            Color::Red
        } else {
            Color::Cyan
        };

        let paragraph = Paragraph::new(lines)
            .block(
                Block::default()
                    .title(self.title)
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(border_color)),
            )
            .alignment(Alignment::Center);

        frame.render_widget(paragraph, area);
    }
}
