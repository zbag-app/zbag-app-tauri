use serde::{Deserialize, Serialize};

use crate::errors;

pub const SCHEMA_VERSION: u32 = 1;

pub type UnixTimestampMs = i64;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VersionedPayload {
    pub schema_version: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IpcError {
    pub code: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<serde_json::Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum IpcResult<T> {
    Ok { ok: T },
    Err { err: IpcError },
}

impl<T> IpcResult<T> {
    pub fn ok(value: T) -> Self {
        Self::Ok { ok: value }
    }

    pub fn err(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self::Err {
            err: IpcError {
                code: code.into(),
                message: message.into(),
                details: None,
            },
        }
    }
}

pub fn ensure_schema_version(schema_version: u32) -> Result<(), IpcError> {
    if schema_version != SCHEMA_VERSION {
        return Err(IpcError {
            code: errors::SCHEMA_VERSION_MISMATCH.to_string(),
            message: format!(
                "schema_version mismatch: expected {}, got {}",
                SCHEMA_VERSION, schema_version
            ),
            details: None,
        });
    }

    Ok(())
}
