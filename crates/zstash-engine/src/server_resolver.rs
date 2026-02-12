use anyhow::Context as _;

use zstash_core::domain::Network;

use crate::db;

pub fn resolve_grpc_url(app_db: &db::AppDb, network: Network) -> anyhow::Result<String> {
    let dev_override = if cfg!(debug_assertions) {
        std::env::var("ZSTASH_GRPC_URL").ok()
    } else {
        None
    };

    resolve_grpc_url_with_dev_override(app_db, network, dev_override.as_deref())
}

/// Resolves the active gRPC endpoint with an optional explicit dev override.
///
/// In release builds, overrides are always ignored.
pub fn resolve_grpc_url_with_dev_override(
    app_db: &db::AppDb,
    network: Network,
    dev_override: Option<&str>,
) -> anyhow::Result<String> {
    // Dev-only override: ignored in release builds to avoid unexpected runtime reconfiguration of
    // network endpoints.
    if cfg!(debug_assertions)
        && let Some(raw_url) = dev_override
    {
        let override_url = raw_url.trim();
        if !override_url.is_empty() {
            crate::grpc_url::validate_grpc_url(override_url).context("invalid ZSTASH_GRPC_URL")?;
            return Ok(override_url.to_string());
        }
    }

    let servers = db::server_meta::list_servers(app_db.conn()).context("failed to list servers")?;
    let server = servers
        .into_iter()
        .find(|s| s.network == network && s.is_default)
        .context("no default server configured for network")?;

    crate::grpc_url::validate_grpc_url(&server.grpc_url).with_context(|| {
        format!(
            "invalid gRPC URL for default {network:?} server '{}' ({})",
            server.name, server.id
        )
    })?;

    Ok(server.grpc_url)
}
