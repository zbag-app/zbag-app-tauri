//! View definitions for the TUI.

#![allow(dead_code)] // Some items are planned for future tasks

pub mod account_details;
pub mod birthday_editor;
pub mod dashboard;
pub mod settings;
pub mod sync_reset;
pub mod unlock;
pub mod wallet_selector;

use ratatui::prelude::*;

use crate::app::AppAction;
use crate::event::KeyAction;

/// Trait for views that can handle events and render themselves.
pub trait View {
    /// Handle a key action and return the resulting app action.
    fn handle_key(&mut self, action: KeyAction) -> AppAction;

    /// Render the view to the frame.
    fn render(&mut self, frame: &mut Frame, area: Rect);
}
