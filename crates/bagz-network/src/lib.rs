#![forbid(unsafe_code)]

//! Transport and network clients (gRPC + HTTP).

pub mod exchange_rate;
pub mod grpc_client;
pub mod http_client;
pub mod near_intents;
mod rustls_provider;
pub mod transport;

pub use rustls_provider::install_ring_crypto_provider;
