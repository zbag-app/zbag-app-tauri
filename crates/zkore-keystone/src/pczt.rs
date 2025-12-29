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
    base64::engine::general_purpose::STANDARD.encode(strip_proprietary(pczt.clone()).serialize())
}

pub fn decode_pczt_base64(payload: &str) -> Result<pczt::Pczt, PcztPayloadError> {
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(payload)
        .map_err(|_| PcztPayloadError::InvalidBase64)?;
    let parsed =
        pczt::Pczt::parse(&bytes).map_err(|e| PcztPayloadError::InvalidPczt(format!("{e:?}")))?;
    Ok(strip_proprietary(parsed))
}

fn strip_proprietary(pczt: pczt::Pczt) -> pczt::Pczt {
    pczt::roles::redactor::Redactor::new(pczt)
        .redact_global_with(|mut global| {
            global.clear_proprietary();
        })
        .redact_transparent_with(|mut transparent| {
            transparent.redact_inputs(|mut input| input.clear_proprietary());
            transparent.redact_outputs(|mut output| output.clear_proprietary());
        })
        .redact_sapling_with(|mut sapling| {
            sapling.redact_spends(|mut spend| spend.clear_proprietary());
            sapling.redact_outputs(|mut output| output.clear_proprietary());
        })
        .redact_orchard_with(|mut orchard| {
            orchard.redact_actions(|mut action| {
                action.clear_spend_proprietary();
                action.clear_output_proprietary();
            });
        })
        .finish()
}
