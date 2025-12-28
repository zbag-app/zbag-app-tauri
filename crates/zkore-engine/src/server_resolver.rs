use anyhow::Context as _;

use zkore_core::domain::Network;

use crate::db;

pub fn resolve_grpc_url(app_db: &db::AppDb, network: Network) -> anyhow::Result<String> {
    if cfg!(debug_assertions) {
        if let Ok(override_url) = std::env::var("ZKORE_GRPC_URL") {
            if !override_url.trim().is_empty() {
                return Ok(override_url);
            }
        }
    }

    let servers = db::server_meta::list_servers(app_db.conn()).context("failed to list servers")?;
    let server = servers
        .into_iter()
        .find(|s| s.network == network && s.is_default)
        .context("no default server configured for network")?;

    Ok(server.grpc_url)
}
