use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::domain::ServerInfo;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AddServerRequest {
    pub schema_version: u32,
    pub name: String,
    pub grpc_url: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SetDefaultServerRequest {
    pub schema_version: u32,
    pub server_id: Uuid,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TestServerRequest {
    pub schema_version: u32,
    pub server_id: Uuid,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ListServersRequest {
    pub schema_version: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AddServerResponse {
    pub schema_version: u32,
    pub server: ServerInfo,
}

/// Response for `SetDefaultServer`.
///
/// # Error handling asymmetry with `TestServerResponse`
///
/// If the stored server configuration is invalid (e.g., malformed `grpc_url`), this command
/// returns an IPC error rather than `{ success: false }`. This is intentional:
///
/// - `SetDefaultServer` is state-changing: the caller expects the default to be set on success.
///   Returning `success: false` would be ambiguous (was it a validation issue or a DB error?).
///   An IPC error signals "operation could not be performed" and lets the frontend display an
///   appropriate error dialog prompting the user to fix the configuration.
///
/// - `TestServer` is a health-check: the purpose is to report server status, including why it
///   might be unhealthy. Returning `success: false` with error details is expected output.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SetDefaultServerResponse {
    pub schema_version: u32,
    pub success: bool,
}

/// Response for `TestServer`.
///
/// This command is a health-check: invalid stored configuration is reported as `success: false`
/// with an `error` message (instead of failing the IPC command). See [`SetDefaultServerResponse`]
/// for an explanation of why state-changing commands handle validation errors differently.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TestServerResponse {
    pub schema_version: u32,
    pub success: bool,
    pub latency_ms: Option<u64>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ListServersResponse {
    pub schema_version: u32,
    pub servers: Vec<ServerInfo>,
}
