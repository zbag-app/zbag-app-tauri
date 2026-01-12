use tauri::Emitter;
use zstash_core::ipc::v1::events::{
    BalanceChangedEvent, SwapChangedEvent, SyncProgressEvent, TorStatusEvent,
    TransactionChangedEvent, WalletStatusEvent,
};

pub const CHANNEL_SYNC: &str = "zstash://sync";
pub const CHANNEL_BALANCE: &str = "zstash://balance";
pub const CHANNEL_TX: &str = "zstash://tx";
pub const CHANNEL_SWAP: &str = "zstash://swap";
pub const CHANNEL_TOR: &str = "zstash://tor";
pub const CHANNEL_WALLET_STATUS: &str = "zstash://wallet-status";

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

pub fn emit_swap_changed(
    app: &tauri::AppHandle,
    event: SwapChangedEvent,
) -> Result<(), tauri::Error> {
    app.emit(CHANNEL_SWAP, event)
}

pub fn emit_tor_status(app: &tauri::AppHandle, event: TorStatusEvent) -> Result<(), tauri::Error> {
    app.emit(CHANNEL_TOR, event)
}

pub fn emit_wallet_status(
    app: &tauri::AppHandle,
    event: WalletStatusEvent,
) -> Result<(), tauri::Error> {
    app.emit(CHANNEL_WALLET_STATUS, event)
}
