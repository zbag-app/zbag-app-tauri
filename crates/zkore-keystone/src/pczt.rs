use base64::Engine as _;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum PcztPayloadError {
    #[error("invalid base64 payload")]
    InvalidBase64,
    #[error("invalid PCZT payload: {0}")]
    InvalidPczt(String),
}

pub fn encode_pczt_base64(pczt: &pczt::Pczt) -> String {
    base64::engine::general_purpose::STANDARD.encode(pczt.serialize())
}

pub fn decode_pczt_base64(payload: &str) -> Result<pczt::Pczt, PcztPayloadError> {
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(payload)
        .map_err(|_| PcztPayloadError::InvalidBase64)?;
    pczt::Pczt::parse(&bytes).map_err(|e| PcztPayloadError::InvalidPczt(format!("{e:?}")))
}
