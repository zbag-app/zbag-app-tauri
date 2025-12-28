use tauri::Emitter;
use zkore_core::ipc::v1::events::{
    BalanceChangedEvent, SyncProgressEvent, TransactionChangedEvent, WalletStatusEvent,
};

pub const CHANNEL_SYNC: &str = "zkore://sync";
pub const CHANNEL_BALANCE: &str = "zkore://balance";
pub const CHANNEL_TX: &str = "zkore://tx";
pub const CHANNEL_WALLET_STATUS: &str = "zkore://wallet-status";

pub fn emit_sync_progress(
    app: &tauri::AppHandle,
    event: SyncProgressEvent,
) -> Result<(), tauri::Error> {
    app.emit(CHANNEL_SYNC, event)
}

pub fn emit_balance_changed(
    app: &tauri::AppHandle,
    event: BalanceChangedEvent,
) -> Result<(), tauri::Error> {
    app.emit(CHANNEL_BALANCE, event)
}

pub fn emit_transaction_changed(
    app: &tauri::AppHandle,
    event: TransactionChangedEvent,
) -> Result<(), tauri::Error> {
    app.emit(CHANNEL_TX, event)
}

pub fn emit_wallet_status(
    app: &tauri::AppHandle,
    event: WalletStatusEvent,
) -> Result<(), tauri::Error> {
    app.emit(CHANNEL_WALLET_STATUS, event)
}
