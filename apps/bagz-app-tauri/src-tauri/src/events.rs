use tauri::Emitter;
use bagz_core::ipc::v1::events::{
    BalanceChangedEvent, JobProgressEvent, ServerFailoverEvent, SwapChangedEvent,
    SyncProgressEvent, TorStatusEvent, TransactionChangedEvent, WalletStatusEvent,
};

pub const CHANNEL_SYNC: &str = "bagz://sync";
pub const CHANNEL_BALANCE: &str = "bagz://balance";
pub const CHANNEL_TX: &str = "bagz://tx";
pub const CHANNEL_SWAP: &str = "bagz://swap";
pub const CHANNEL_TOR: &str = "bagz://tor";
pub const CHANNEL_WALLET_STATUS: &str = "bagz://wallet-status";
pub const CHANNEL_JOB: &str = "bagz://job";
pub const CHANNEL_SERVER_FAILOVER: &str = "bagz://server-failover";

pub fn emit_sync_progress(
    app: &crate::AppHandle,
    event: SyncProgressEvent,
) -> Result<(), tauri::Error> {
    app.emit(CHANNEL_SYNC, event)
}

pub fn emit_balance_changed(
    app: &crate::AppHandle,
    event: BalanceChangedEvent,
) -> Result<(), tauri::Error> {
    app.emit(CHANNEL_BALANCE, event)
}

pub fn emit_transaction_changed(
    app: &crate::AppHandle,
    event: TransactionChangedEvent,
) -> Result<(), tauri::Error> {
    app.emit(CHANNEL_TX, event)
}

pub fn emit_swap_changed(
    app: &crate::AppHandle,
    event: SwapChangedEvent,
) -> Result<(), tauri::Error> {
    app.emit(CHANNEL_SWAP, event)
}

pub fn emit_tor_status(app: &crate::AppHandle, event: TorStatusEvent) -> Result<(), tauri::Error> {
    app.emit(CHANNEL_TOR, event)
}

pub fn emit_wallet_status(
    app: &crate::AppHandle,
    event: WalletStatusEvent,
) -> Result<(), tauri::Error> {
    app.emit(CHANNEL_WALLET_STATUS, event)
}

pub fn emit_job_progress(
    app: &crate::AppHandle,
    event: JobProgressEvent,
) -> Result<(), tauri::Error> {
    app.emit(CHANNEL_JOB, event)
}

pub fn emit_server_failover(
    app: &crate::AppHandle,
    event: ServerFailoverEvent,
) -> Result<(), tauri::Error> {
    app.emit(CHANNEL_SERVER_FAILOVER, event)
}
