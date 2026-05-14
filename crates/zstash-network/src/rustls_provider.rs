use std::sync::Once;

static INSTALL_RING_PROVIDER: Once = Once::new();

pub fn install_ring_crypto_provider() {
    INSTALL_RING_PROVIDER.call_once(|| {
        if rustls::crypto::ring::default_provider()
            .install_default()
            .is_err()
        {
            panic!(
                "failed to install ring as rustls default CryptoProvider before TLS initialization"
            );
        }
    });
}

#[cfg(test)]
mod tests {
    #[test]
    fn ring_provider_installs_and_is_default() {
        super::install_ring_crypto_provider();
        let provider = rustls::crypto::CryptoProvider::get_default()
            .expect("default rustls provider must be installed");
        assert!(!provider.cipher_suites.is_empty());
        assert_eq!(
            provider.signature_verification_algorithms.mapping.len(),
            rustls::crypto::ring::default_provider()
                .signature_verification_algorithms
                .mapping
                .len()
        );
    }
}
