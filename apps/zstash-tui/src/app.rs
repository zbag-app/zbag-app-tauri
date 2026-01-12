#![allow(dead_code)] // Many items are planned for future tasks

use std::path::PathBuf;

use zstash_core::domain::{Network, WalletInfo};

use crate::views::birthday_editor::BirthdayEditorState;
use crate::views::dashboard::DashboardData;
use crate::views::settings::SettingsState;
use crate::views::sync_reset::SyncResetState;

/// The current state of the application
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum AppState {
    /// Selecting a wallet from the list
    #[default]
    WalletSelection,
    /// Entering password to unlock a wallet
    PasswordEntry,
    /// Main dashboard view with tabs
    Dashboard { tab: DashboardTab },
    /// Viewing details of a specific account
    AccountDetails { account_id: u32 },
    /// Editing birthday height for an account
    BirthdayEditor { account_id: u32 },
    /// Resetting sync state
    SyncReset,
    /// Editing wallet settings
    Settings,
    /// Showing a confirmation dialog
    Confirmation {
        action: ConfirmAction,
        previous: Box<AppState>,
    },
}

/// Dashboard tabs
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DashboardTab {
    #[default]
    Overview,
    Accounts,
    Sync,
    Settings,
}

impl DashboardTab {
    pub fn next(self) -> Self {
        match self {
            Self::Overview => Self::Accounts,
            Self::Accounts => Self::Sync,
            Self::Sync => Self::Settings,
            Self::Settings => Self::Overview,
        }
    }

    pub fn prev(self) -> Self {
        match self {
            Self::Overview => Self::Settings,
            Self::Accounts => Self::Overview,
            Self::Sync => Self::Accounts,
            Self::Settings => Self::Sync,
        }
    }

    pub fn title(self) -> &'static str {
        match self {
            Self::Overview => "Overview",
            Self::Accounts => "Accounts",
            Self::Sync => "Sync",
            Self::Settings => "Settings",
        }
    }
}

/// Actions that require confirmation
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfirmAction {
    /// Update birthday height to a new value
    UpdateBirthday { account_id: u32, new_height: u32 },
    /// Reset sync state
    ResetSync,
}

/// Result of handling an event
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AppAction {
    /// No action needed
    None,
    /// Quit the application
    Quit,
    /// Transition to a new state
    ChangeState(AppState),
    /// Show an error message
    ShowError(String),
}

/// Configuration paths for the app
#[derive(Debug, Clone)]
pub struct AppConfig {
    pub app_db_path: PathBuf,
    pub wallets_dir: PathBuf,
}

impl AppConfig {
    pub fn new(app_db_path: PathBuf, wallets_dir: PathBuf) -> Self {
        Self {
            app_db_path,
            wallets_dir,
        }
    }

    pub fn from_defaults() -> Option<Self> {
        let home = std::env::var_os("HOME")?;
        let home = PathBuf::from(home);
        Some(Self {
            app_db_path: home.join(".zstash").join("app.db"),
            wallets_dir: home.join(".zstash").join("wallets"),
        })
    }
}

/// The main application state
pub struct App {
    /// Current application state
    pub state: AppState,
    /// Configuration paths
    pub config: AppConfig,
    /// List of available wallets
    pub wallets: Vec<WalletInfo>,
    /// Currently selected wallet index in the list
    pub selected_wallet_idx: usize,
    /// Currently selected/unlocked wallet
    pub current_wallet: Option<WalletInfo>,
    /// Password input buffer (for PasswordEntry state)
    pub password_input: String,
    /// Error message to display
    pub error_message: Option<String>,
    /// Whether the app should quit
    pub should_quit: bool,
    /// Whether to show the help overlay
    pub show_help: bool,
    /// Cached dashboard data (loaded after unlock)
    pub dashboard_data: DashboardData,
    /// Birthday editor state
    pub birthday_editor_state: BirthdayEditorState,
    /// Sync reset state
    pub sync_reset_state: SyncResetState,
    /// Settings state
    pub settings_state: SettingsState,
    /// Cached DEK bytes for write operations (stored after successful unlock)
    /// Note: This is the raw key material - handle with care
    pub cached_dek_bytes: Option<[u8; 32]>,
}

impl App {
    pub fn new(config: AppConfig) -> Self {
        Self {
            state: AppState::WalletSelection,
            config,
            wallets: Vec::new(),
            selected_wallet_idx: 0,
            current_wallet: None,
            password_input: String::new(),
            error_message: None,
            should_quit: false,
            show_help: false,
            dashboard_data: DashboardData::default(),
            birthday_editor_state: BirthdayEditorState::default(),
            sync_reset_state: SyncResetState::default(),
            settings_state: SettingsState::default(),
            cached_dek_bytes: None,
        }
    }

    /// Transition to a new state
    pub fn go_to(&mut self, state: AppState) {
        self.state = state;
        self.error_message = None;
    }

    /// Go back to the previous logical state
    pub fn go_back(&mut self) {
        match &self.state {
            AppState::WalletSelection => {
                // Can't go back from here, quit instead
                self.should_quit = true;
            }
            AppState::PasswordEntry => {
                self.password_input.clear();
                self.go_to(AppState::WalletSelection);
            }
            AppState::Dashboard { .. } => {
                // Lock wallet and go back to selection
                self.current_wallet = None;
                self.cached_dek_bytes = None; // Clear cached DEK when locking
                self.go_to(AppState::WalletSelection);
            }
            AppState::AccountDetails { .. } => {
                self.go_to(AppState::Dashboard {
                    tab: DashboardTab::Accounts,
                });
            }
            AppState::BirthdayEditor { .. } => {
                // Go back to account details - but we don't have the account_id here
                // In practice, the view will handle this
                self.go_to(AppState::Dashboard {
                    tab: DashboardTab::Accounts,
                });
            }
            AppState::SyncReset => {
                self.go_to(AppState::Dashboard {
                    tab: DashboardTab::Sync,
                });
            }
            AppState::Settings => {
                self.go_to(AppState::Dashboard {
                    tab: DashboardTab::Settings,
                });
            }
            AppState::Confirmation { previous, .. } => {
                self.state = (**previous).clone();
            }
        }
    }

    /// Set an error message to display
    pub fn set_error(&mut self, msg: impl Into<String>) {
        self.error_message = Some(msg.into());
    }

    /// Clear the error message
    pub fn clear_error(&mut self) {
        self.error_message = None;
    }

    /// Select next wallet in the list
    pub fn select_next_wallet(&mut self) {
        if !self.wallets.is_empty() {
            self.selected_wallet_idx = (self.selected_wallet_idx + 1) % self.wallets.len();
        }
    }

    /// Select previous wallet in the list
    pub fn select_prev_wallet(&mut self) {
        if !self.wallets.is_empty() {
            self.selected_wallet_idx = if self.selected_wallet_idx == 0 {
                self.wallets.len() - 1
            } else {
                self.selected_wallet_idx - 1
            };
        }
    }

    /// Get the currently selected wallet (if any)
    pub fn selected_wallet(&self) -> Option<&WalletInfo> {
        self.wallets.get(self.selected_wallet_idx)
    }

    /// Switch to the next dashboard tab
    pub fn next_tab(&mut self) {
        if let AppState::Dashboard { tab } = &mut self.state {
            *tab = tab.next();
        }
    }

    /// Switch to the previous dashboard tab
    pub fn prev_tab(&mut self) {
        if let AppState::Dashboard { tab } = &mut self.state {
            *tab = tab.prev();
        }
    }
}

// Suppress unused warnings for types that will be used in later tasks
#[allow(dead_code)]
const _: () = {
    fn _use_types() {
        let _ = std::mem::size_of::<Network>();
        let _ = std::mem::size_of::<AppAction>();
    }
};
