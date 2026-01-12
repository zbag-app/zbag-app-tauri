use zkore_core::errors;
use zkore_core::ipc::v1::common::{IpcError, IpcResult};
use zkore_engine::error::find_engine_ipc_error;

pub fn map_anyhow<T, F>(f: F) -> IpcResult<T>
where
    F: FnOnce() -> anyhow::Result<T>,
{
    match f() {
        Ok(value) => IpcResult::ok(value),
        Err(err) => IpcResult::Err {
            err: to_ipc_error(err),
        },
    }
}

pub fn to_ipc_error(err: anyhow::Error) -> IpcError {
    if let Some(engine) = find_engine_ipc_error(&err) {
        return IpcError {
            code: engine.code.to_string(),
            message: engine.message.clone(),
            details: engine.details.clone(),
        };
    }

    IpcError {
        code: errors::INTERNAL_ERROR.to_string(),
        message: "internal error".to_string(),
        details: None,
    }
}

pub fn system_time_to_unix_ms(time: std::time::SystemTime) -> anyhow::Result<i64> {
    let duration = time.duration_since(std::time::UNIX_EPOCH)?;
    Ok(i64::try_from(duration.as_millis())?)
}
