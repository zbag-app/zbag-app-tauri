#![allow(dead_code)] // Some items are planned for future tasks

use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use std::time::Duration;

/// Result of polling for an event
pub enum EventResult {
    /// A key was pressed
    Key(KeyAction),
    /// No event (timeout)
    Tick,
    /// Terminal was resized
    Resize(u16, u16),
}

/// Semantic key actions for the application
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyAction {
    /// Quit the application (Ctrl+C or Ctrl+Q only from keyboard, views handle 'q')
    Quit,
    /// Navigate up in a list
    Up,
    /// Navigate down in a list
    Down,
    /// Select/confirm current item
    Select,
    /// Go back/cancel
    Back,
    /// Switch to next tab
    NextTab,
    /// Switch to previous tab
    PrevTab,
    /// A character was typed (for text input AND single-key shortcuts)
    Char(char),
    /// Backspace (for text input)
    Backspace,
    /// Delete (for text input)
    Delete,
    /// Unknown/unhandled key
    Unknown,
}

impl From<KeyEvent> for KeyAction {
    fn from(key: KeyEvent) -> Self {
        // Only handle key press events, not release
        if key.kind != KeyEventKind::Press {
            return Self::Unknown;
        }

        match key.code {
            // Ctrl+C always quits
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => Self::Quit,
            // Ctrl+Q also quits
            KeyCode::Char('q') if key.modifiers.contains(KeyModifiers::CONTROL) => Self::Quit,

            // Navigation keys (not affected by text input)
            KeyCode::Up => Self::Up,
            KeyCode::Down => Self::Down,
            KeyCode::Enter => Self::Select,
            KeyCode::Esc => Self::Back,
            KeyCode::Tab => Self::NextTab,
            KeyCode::BackTab => Self::PrevTab,

            // Text editing
            KeyCode::Backspace => Self::Backspace,
            KeyCode::Delete => Self::Delete,

            // All characters - views decide if shortcuts
            KeyCode::Char(c) => Self::Char(c),

            _ => Self::Unknown,
        }
    }
}

/// Poll for the next event with a timeout
pub fn poll_event(timeout: Duration) -> std::io::Result<EventResult> {
    if event::poll(timeout)? {
        match event::read()? {
            Event::Key(key) => Ok(EventResult::Key(KeyAction::from(key))),
            Event::Resize(width, height) => Ok(EventResult::Resize(width, height)),
            _ => Ok(EventResult::Tick),
        }
    } else {
        Ok(EventResult::Tick)
    }
}

/// Check if a key action is a character input (for password/text fields)
impl KeyAction {
    pub fn is_text_input(&self) -> bool {
        matches!(self, Self::Char(_) | Self::Backspace | Self::Delete)
    }

    /// Get the character if this is a Char action
    pub fn as_char(&self) -> Option<char> {
        match self {
            Self::Char(c) => Some(*c),
            _ => None,
        }
    }
}
