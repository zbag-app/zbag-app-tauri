use tauri::{Emitter, Runtime};
use zbag_core::ipc::v1::events::{
    BalanceChangedEvent, JobProgressEvent, ServerFailoverEvent, SwapChangedEvent,
    SyncProgressEvent, TorStatusEvent, TransactionChangedEvent, WalletStatusEvent,
};

pub const CHANNEL_SYNC: &str = "zbag://sync";
pub const CHANNEL_BALANCE: &str = "zbag://balance";
pub const CHANNEL_TX: &str = "zbag://tx";
pub const CHANNEL_SWAP: &str = "zbag://swap";
pub const CHANNEL_TOR: &str = "zbag://tor";
pub const CHANNEL_WALLET_STATUS: &str = "zbag://wallet-status";
pub const CHANNEL_JOB: &str = "zbag://job";
pub const CHANNEL_SERVER_FAILOVER: &str = "zbag://server-failover";

pub fn emit_sync_progress<R: Runtime>(
    app: &tauri::AppHandle<R>,
    event: SyncProgressEvent,
) -> Result<(), tauri::Error> {
    app.emit(CHANNEL_SYNC, event)
}

pub fn emit_balance_changed<R: Runtime>(
    app: &tauri::AppHandle<R>,
    event: BalanceChangedEvent,
) -> Result<(), tauri::Error> {
    app.emit(CHANNEL_BALANCE, event)
}

pub fn emit_transaction_changed<R: Runtime>(
    app: &tauri::AppHandle<R>,
    event: TransactionChangedEvent,
) -> Result<(), tauri::Error> {
    app.emit(CHANNEL_TX, event)
}

pub fn emit_swap_changed<R: Runtime>(
    app: &tauri::AppHandle<R>,
    event: SwapChangedEvent,
) -> Result<(), tauri::Error> {
    app.emit(CHANNEL_SWAP, event)
}

pub fn emit_tor_status<R: Runtime>(
    app: &tauri::AppHandle<R>,
    event: TorStatusEvent,
) -> Result<(), tauri::Error> {
    app.emit(CHANNEL_TOR, event)
}

pub fn emit_wallet_status<R: Runtime>(
    app: &tauri::AppHandle<R>,
    event: WalletStatusEvent,
) -> Result<(), tauri::Error> {
    app.emit(CHANNEL_WALLET_STATUS, event)
}

pub fn emit_job_progress<R: Runtime>(
    app: &tauri::AppHandle<R>,
    event: JobProgressEvent,
) -> Result<(), tauri::Error> {
    app.emit(CHANNEL_JOB, event)
}

pub fn emit_server_failover<R: Runtime>(
    app: &tauri::AppHandle<R>,
    event: ServerFailoverEvent,
) -> Result<(), tauri::Error> {
    app.emit(CHANNEL_SERVER_FAILOVER, event)
}
