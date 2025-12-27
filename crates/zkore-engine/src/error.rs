use serde_json::Value;

#[derive(Debug, thiserror::Error)]
#[error("{code}: {message}")]
pub struct EngineIpcError {
    pub code: &'static str,
    pub message: String,
    pub details: Option<Value>,
}

impl EngineIpcError {
    pub fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            details: None,
        }
    }

    pub fn with_details(mut self, details: Value) -> Self {
        self.details = Some(details);
        self
    }
}

pub fn ipc_err(code: &'static str, message: impl Into<String>) -> anyhow::Error {
    anyhow::anyhow!(EngineIpcError::new(code, message))
}

pub fn find_engine_ipc_error(err: &anyhow::Error) -> Option<&EngineIpcError> {
    err.chain().find_map(|cause| cause.downcast_ref::<EngineIpcError>())
}

