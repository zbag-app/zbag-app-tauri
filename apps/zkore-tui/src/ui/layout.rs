//! Layout helpers for the TUI.

#![allow(dead_code)] // Some layouts are planned for future tasks

use ratatui::prelude::*;
use ratatui::widgets::Paragraph;

/// Create a centered rect using a percentage of the available area.
pub fn centered_rect(percent_x: u16, percent_y: u16, area: Rect) -> Rect {
    let popup_layout = Layout::vertical([
        Constraint::Percentage((100 - percent_y) / 2),
        Constraint::Percentage(percent_y),
        Constraint::Percentage((100 - percent_y) / 2),
    ])
    .split(area);

    Layout::horizontal([
        Constraint::Percentage((100 - percent_x) / 2),
        Constraint::Percentage(percent_x),
        Constraint::Percentage((100 - percent_x) / 2),
    ])
    .split(popup_layout[1])[1]
}

/// Create a centered rect with fixed dimensions.
pub fn centered_rect_fixed(width: u16, height: u16, area: Rect) -> Rect {
    let x = area.x + (area.width.saturating_sub(width)) / 2;
    let y = area.y + (area.height.saturating_sub(height)) / 2;
    Rect::new(x, y, width.min(area.width), height.min(area.height))
}

/// Split an area into header, main content, and footer.
pub fn main_layout(area: Rect) -> (Rect, Rect, Rect) {
    let chunks = Layout::vertical([
        Constraint::Length(3), // Header
        Constraint::Min(0),    // Main content
        Constraint::Length(1), // Footer
    ])
    .split(area);
    (chunks[0], chunks[1], chunks[2])
}

/// Create horizontal tabs layout.
pub fn tabs_layout(area: Rect) -> (Rect, Rect) {
    let chunks = Layout::vertical([
        Constraint::Length(3), // Tabs bar
        Constraint::Min(0),    // Content
    ])
    .split(area);
    (chunks[0], chunks[1])
}

/// Render a status bar at the bottom of the screen.
pub fn render_status_bar(
    frame: &mut Frame,
    area: Rect,
    wallet_name: Option<&str>,
    network: Option<&str>,
) {
    let status = match (wallet_name, network) {
        (Some(name), Some(net)) => format!(" {} | {} | Press ? for help", name, net),
        (Some(name), None) => format!(" {} | Press ? for help", name),
        _ => " Press ? for help".to_string(),
    };

    let paragraph =
        Paragraph::new(status).style(Style::default().fg(Color::White).bg(Color::DarkGray));
    frame.render_widget(paragraph, area);
}
