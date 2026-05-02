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
