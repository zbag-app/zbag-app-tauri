use thiserror::Error;
use zcash_protocol::consensus::{Network, NetworkType, Parameters as _};

#[allow(deprecated)]
use zcash_client_backend::keys::UnifiedFullViewingKey;

#[derive(Debug, Clone)]
pub struct ParsedUfvk {
    pub network: NetworkType,
    pub ufvk: UnifiedFullViewingKey,
}

#[derive(Debug, Error)]
pub enum UfvkError {
    #[error("{0}")]
    Invalid(String),
}

pub fn parse_ufvk(encoding: &str) -> Result<ParsedUfvk, UfvkError> {
    let mut last_err: Option<String> = None;
    for net in [Network::MainNetwork, Network::TestNetwork] {
        match UnifiedFullViewingKey::decode(&net, encoding) {
            Ok(ufvk) => {
                return Ok(ParsedUfvk {
                    network: net.network_type(),
                    ufvk,
                });
            }
            Err(err) => {
                last_err = Some(err);
            }
        }
    }

    Err(UfvkError::Invalid(
        last_err.unwrap_or_else(|| "invalid UFVK".to_string()),
    ))
}
