use base64::Engine as _;
use thiserror::Error;
use tracing::{debug, error, instrument};

#[derive(Debug, Error)]
pub enum PcztPayloadError {
    #[error("invalid base64 payload")]
    InvalidBase64,
    #[error("invalid PCZT payload: {0}")]
    InvalidPczt(String),
}

/// Encode a PCZT for sending to a hardware signer (e.g., Keystone).
///
/// This performs selective redaction matching Zashi's approach:
/// - Redact proprietary `zcash_client_backend:proposal_info` from global
/// - Clear spend witnesses and redact `zcash_client_backend:output_info` from shielded bundles
/// - Keep other PCZT data intact (including zip32_derivation for signing)
///
/// The full PCZT with proofs should be kept in the backend for combining later.
pub fn encode_pczt_for_signer(pczt: &pczt::Pczt) -> String {
    base64::engine::general_purpose::STANDARD.encode(redact_for_signer(pczt.clone()).serialize())
}

/// Encode a PCZT without any redaction (for backend storage).
pub fn encode_pczt_full(pczt: &pczt::Pczt) -> String {
    base64::engine::general_purpose::STANDARD.encode(pczt.serialize())
}

/// Encode a PCZT for sending to signer, with the old strip_proprietary behavior.
/// Deprecated: Use encode_pczt_for_signer for Keystone signing.
pub fn encode_pczt_base64(pczt: &pczt::Pczt) -> String {
    // Use the new selective redaction for signer
    encode_pczt_for_signer(pczt)
}

/// Decode a signed PCZT from a hardware signer.
#[instrument(skip(payload), fields(payload_len = payload.len()))]
pub fn decode_pczt_base64(payload: &str) -> Result<pczt::Pczt, PcztPayloadError> {
    debug!("Decoding base64 PCZT payload");
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(payload)
        .map_err(|e| {
            error!("Invalid base64: {}", e);
            PcztPayloadError::InvalidBase64
        })?;

    debug!(bytes_len = bytes.len(), "Parsing PCZT from bytes");
    let parsed = pczt::Pczt::parse(&bytes).map_err(|e| {
        error!("Failed to parse PCZT: {:?}", e);
        PcztPayloadError::InvalidPczt(format!("{e:?}"))
    })?;

    debug!("PCZT decoded successfully");
    Ok(parsed)
}

/// Decode a full PCZT (with proofs) from base64.
#[instrument(skip(payload), fields(payload_len = payload.len()))]
pub fn decode_pczt_full(payload: &str) -> Result<pczt::Pczt, PcztPayloadError> {
    debug!("Decoding full PCZT payload");
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(payload)
        .map_err(|e| {
            error!("Invalid base64: {}", e);
            PcztPayloadError::InvalidBase64
        })?;

    debug!(bytes_len = bytes.len(), "Parsing full PCZT from bytes");
    let parsed = pczt::Pczt::parse(&bytes).map_err(|e| {
        error!("Failed to parse full PCZT: {:?}", e);
        PcztPayloadError::InvalidPczt(format!("{e:?}"))
    })?;

    debug!("Full PCZT decoded successfully");
    Ok(parsed)
}

/// Selectively redact a PCZT for sending to a hardware signer.
///
/// This matches Zashi's `zcashlc_redact_pczt_for_signer`:
/// - Redact `zcash_client_backend:proposal_info` from global (not needed by signer)
/// - Clear spend witnesses (signer adds signatures, not proofs)
/// - Redact `zcash_client_backend:output_info` from outputs (not needed by signer)
///
/// Critically, this does NOT strip all proprietary data, so `extract_and_store_transaction_from_pczt`
/// can still work when combining with the proved PCZT.
fn redact_for_signer(pczt: pczt::Pczt) -> pczt::Pczt {
    pczt::roles::redactor::Redactor::new(pczt)
        .redact_global_with(|mut r| {
            r.redact_proprietary("zcash_client_backend:proposal_info");
        })
        .redact_orchard_with(|mut r| {
            r.redact_actions(|mut ar| {
                ar.clear_spend_witness();
                ar.redact_output_proprietary("zcash_client_backend:output_info");
            });
        })
        .redact_sapling_with(|mut r| {
            r.redact_spends(|mut sr| sr.clear_witness());
            r.redact_outputs(|mut or| {
                or.redact_proprietary("zcash_client_backend:output_info");
            });
        })
        .redact_transparent_with(|mut r| {
            r.redact_outputs(|mut or| {
                or.redact_proprietary("zcash_client_backend:output_info");
            });
        })
        .finish()
}

/// Combine a proved PCZT (with proofs) and a signed PCZT (with signatures) into one.
///
/// This is used in the two-PCZT flow:
/// 1. Backend creates PCZT and generates proofs, stores full PCZT
/// 2. Redacted PCZT is sent to Keystone for signing
/// 3. Signed PCZT comes back from Keystone
/// 4. This function combines the two to get a PCZT with both proofs and signatures
#[instrument(skip_all)]
pub fn combine_pczts(
    pczt_with_proofs: pczt::Pczt,
    pczt_with_sigs: pczt::Pczt,
) -> Result<pczt::Pczt, PcztPayloadError> {
    debug!("Combining proved PCZT with signed PCZT");
    let combined = pczt::roles::combiner::Combiner::new(vec![pczt_with_proofs, pczt_with_sigs])
        .combine()
        .map_err(|e| {
            error!("Failed to combine PCZTs: {:?}", e);
            PcztPayloadError::InvalidPczt(format!("failed to combine PCZTs: {e:?}"))
        })?;

    debug!("PCZTs combined successfully");
    Ok(combined)
}
