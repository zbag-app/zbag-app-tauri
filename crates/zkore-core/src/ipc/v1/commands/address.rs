use serde::{Deserialize, Serialize};

use crate::domain::{AddressInfo, AddressType};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GetReceiveAddressRequest {
    pub schema_version: u32,
    pub account_id: u32,
    pub address_type: AddressType,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GetReceiveAddressResponse {
    pub schema_version: u32,
    pub address: AddressInfo,
}
