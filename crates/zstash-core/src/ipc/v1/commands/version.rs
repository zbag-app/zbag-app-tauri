use serde::{Deserialize, Serialize};

use crate::version::VersionInfo;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GetVersionRequest {
    pub schema_version: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GetVersionResponse {
    pub schema_version: u32,
    pub version_info: VersionInfo,
}
