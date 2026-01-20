//! Lightwalletd server management commands.

use std::path::Path;
use std::time::Instant;

use anyhow::Result;
use clap::{Args, Subcommand};
use console::style;
use uuid::Uuid;

use zstash_core::domain::{Network, ServerInfo};
use zstash_engine::error::find_engine_ipc_error;
use zstash_engine::grpc_url::validate_grpc_url;
use zstash_network::grpc_client::GrpcClient;

use crate::cli_app_state::CliAppState;
use crate::output::OutputMode;

#[derive(Args)]
pub struct ServerArgs {
    #[command(subcommand)]
    command: ServerCommand,
}

#[derive(Subcommand)]
enum ServerCommand {
    /// List configured servers
    List,

    /// Add a new lightwalletd server
    Add {
        /// Server display name
        #[arg(long)]
        name: String,

        /// gRPC URL (e.g., https://lwd.mainnet.example.com)
        #[arg(long)]
        url: String,
    },

    /// Set the default server for a network
    SetDefault {
        /// Server ID (use 'server list' to see IDs)
        server_id: String,
    },

    /// Test server connectivity
    Test {
        /// Server ID (use 'server list' to see IDs)
        server_id: String,
    },
}

pub async fn run(args: ServerArgs, data_dir: &Path, output: &OutputMode) -> Result<()> {
    match args.command {
        ServerCommand::List => list_servers(data_dir, output).await,
        ServerCommand::Add { name, url } => add_server(data_dir, &name, &url, output).await,
        ServerCommand::SetDefault { server_id } => {
            set_default_server(data_dir, &server_id, output).await
        }
        ServerCommand::Test { server_id } => test_server(data_dir, &server_id, output).await,
    }
}

async fn list_servers(data_dir: &Path, output: &OutputMode) -> Result<()> {
    let state = CliAppState::new(data_dir, false)?;

    let servers = {
        let wm = state.wallet_manager.lock().expect("mutex poisoned");
        zstash_engine::db::server_meta::list_servers(wm.app_db().conn())
            .map_err(|e| anyhow::anyhow!(e))?
    };
    // Intentionally do not validate URLs here: this is a read-only listing, and invalid/legacy
    // stored entries are rejected when used (set default, test, resolve).

    print_server_list(output, &servers);
    Ok(())
}

async fn add_server(data_dir: &Path, name: &str, url: &str, output: &OutputMode) -> Result<()> {
    let name = name.trim();
    if name.is_empty() {
        anyhow::bail!("server name is required");
    }

    let url = url.trim();
    if url.is_empty() {
        anyhow::bail!("gRPC URL is required");
    }

    // Validate URL format and scheme policy (release: HTTPS only; debug: allow HTTP localhost)
    validate_grpc_url(url)?;

    let state = CliAppState::new(data_dir, false)?;

    // Probe server to verify connectivity and get network
    if !output.is_json() {
        println!("Probing server {}...", style(url).cyan());
    }

    let client = GrpcClient::new(url.to_string());

    let started = Instant::now();
    let info = client
        .probe_server()
        .await
        .map_err(|e| anyhow::anyhow!("server probe failed: {}", e))?;
    let latency_ms = started.elapsed().as_millis() as u64;

    let network = parse_network(&info.chain_name)?;

    let now_ms = chrono::Utc::now().timestamp_millis();
    let server = ServerInfo {
        id: Uuid::new_v4(),
        name: name.to_string(),
        grpc_url: url.to_string(),
        network,
        is_default: false,
        last_success_at: Some(now_ms),
        validation_error: None,
    };

    {
        let wm = state.wallet_manager.lock().expect("mutex poisoned");
        zstash_engine::db::server_meta::insert_server(wm.app_db().conn(), &server, now_ms)
            .map_err(|e| anyhow::anyhow!(e))?;
    }

    print_server_added(output, &server, latency_ms);
    Ok(())
}

async fn set_default_server(
    data_dir: &Path,
    server_id_str: &str,
    output: &OutputMode,
) -> Result<()> {
    let state = CliAppState::new(data_dir, false)?;

    let server_id = parse_server_id(&state, server_id_str)?;

    let server = {
        let wm = state.wallet_manager.lock().expect("mutex poisoned");
        zstash_engine::db::server_meta::get_server(wm.app_db().conn(), server_id)
            .map_err(|e| anyhow::anyhow!(e))?
            .ok_or_else(|| anyhow::anyhow!("server not found: {}", server_id_str))?
    };

    // Defense-in-depth: stored values may be tampered with or come from legacy versions.
    validate_grpc_url(&server.grpc_url)?;

    {
        let mut wm = state.wallet_manager.lock().expect("mutex poisoned");
        zstash_engine::db::server_meta::set_default_server(wm.app_db_mut().conn_mut(), server_id)
            .map_err(|e| anyhow::anyhow!(e))?;
    }

    print_default_set(output, &server);
    Ok(())
}

async fn test_server(data_dir: &Path, server_id_str: &str, output: &OutputMode) -> Result<()> {
    let state = CliAppState::new(data_dir, false)?;

    let server_id = parse_server_id(&state, server_id_str)?;

    let server = {
        let wm = state.wallet_manager.lock().expect("mutex poisoned");
        zstash_engine::db::server_meta::get_server(wm.app_db().conn(), server_id)
            .map_err(|e| anyhow::anyhow!(e))?
            .ok_or_else(|| anyhow::anyhow!("server not found: {}", server_id_str))?
    };

    // Validate URL format and scheme policy (release: HTTPS only; debug: allow HTTP localhost).
    // `test_server` is a health-check, so invalid stored configuration is reported as a test
    // failure rather than failing the CLI command itself.
    if let Err(err) = validate_grpc_url(&server.grpc_url) {
        let message = find_engine_ipc_error(&err)
            .map(|engine| engine.message.clone())
            .unwrap_or_else(|| err.to_string());
        print_test_result(
            output,
            &server,
            false,
            None,
            Some(format!("stored server configuration is invalid: {message}")),
        );
        return Ok(());
    }

    if !output.is_json() {
        println!("Testing server {}...", style(&server.grpc_url).cyan());
    }

    let client = GrpcClient::new(server.grpc_url.clone());

    let started = Instant::now();
    let probe_result = client.probe_server().await;
    let latency_ms = started.elapsed().as_millis() as u64;

    match probe_result {
        Ok(info) => {
            let probed_network = parse_network(&info.chain_name)?;
            if probed_network != server.network {
                print_test_result(
                    output,
                    &server,
                    false,
                    Some(latency_ms),
                    Some("server network mismatch".to_string()),
                );
                return Ok(());
            }

            // Update last_success_at
            let now_ms = chrono::Utc::now().timestamp_millis();
            {
                let wm = state.wallet_manager.lock().expect("mutex poisoned");
                let _ = zstash_engine::db::server_meta::update_last_success_at(
                    wm.app_db().conn(),
                    server.id,
                    now_ms,
                );
            }

            print_test_result(output, &server, true, Some(latency_ms), None);
        }
        Err(err) => {
            print_test_result(
                output,
                &server,
                false,
                Some(latency_ms),
                Some(err.to_string()),
            );
        }
    }

    Ok(())
}

/// Parse chain_name from lightwalletd into our Network type.
fn parse_network(chain_name: &str) -> Result<Network> {
    let name = chain_name.trim().to_lowercase();
    match name.as_str() {
        "main" | "mainnet" => Ok(Network::Mainnet),
        "test" | "testnet" => Ok(Network::Testnet),
        other => anyhow::bail!("unsupported chain_name: {}", other),
    }
}

/// Parse a server ID from string. Supports full UUID or prefix matching.
fn parse_server_id(state: &CliAppState, id_str: &str) -> Result<Uuid> {
    // Try to parse as full UUID first
    if let Ok(uuid) = Uuid::parse_str(id_str) {
        return Ok(uuid);
    }

    // Otherwise, try prefix matching
    let servers = {
        let wm = state.wallet_manager.lock().expect("mutex poisoned");
        zstash_engine::db::server_meta::list_servers(wm.app_db().conn())
            .map_err(|e| anyhow::anyhow!(e))?
    };

    let id_lower = id_str.to_lowercase();
    let matches: Vec<_> = servers
        .iter()
        .filter(|s| s.id.to_string().to_lowercase().starts_with(&id_lower))
        .collect();

    match matches.len() {
        0 => anyhow::bail!("server not found: {}", id_str),
        1 => Ok(matches[0].id),
        n => anyhow::bail!(
            "ambiguous server ID prefix '{}' matches {} servers",
            id_str,
            n
        ),
    }
}

// Output formatting functions

fn print_server_list(output: &OutputMode, servers: &[ServerInfo]) {
    if output.is_json() {
        println!("{}", serde_json::to_string_pretty(&servers).unwrap());
    } else {
        if servers.is_empty() {
            println!("No servers configured.");
            println!();
            println!(
                "Add one with: {} server add --name <NAME> --url <URL>",
                style("zstash").cyan()
            );
            return;
        }

        println!("{}", style("Servers:").bold());
        for server in servers {
            let short_id = &server.id.to_string()[..8];
            let network_style = match server.network {
                Network::Mainnet => style("mainnet").green(),
                Network::Testnet => style("testnet").yellow(),
            };
            let default_marker = if server.is_default {
                style(" [default]").green().to_string()
            } else {
                String::new()
            };
            println!(
                "  {} {} [{}]{}",
                style(short_id).cyan(),
                server.name,
                network_style,
                default_marker
            );
            println!("       {}", style(&server.grpc_url).dim());
        }
    }
}

fn print_server_added(output: &OutputMode, server: &ServerInfo, latency_ms: u64) {
    if output.is_json() {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "server": server,
                "latency_ms": latency_ms
            }))
            .unwrap()
        );
    } else {
        let network_style = match server.network {
            Network::Mainnet => style("mainnet").green(),
            Network::Testnet => style("testnet").yellow(),
        };
        println!("{}", style("Server added successfully!").green().bold());
        println!();
        println!("  ID:      {}", style(&server.id.to_string()[..8]).cyan());
        println!("  Name:    {}", server.name);
        println!("  URL:     {}", server.grpc_url);
        println!("  Network: {}", network_style);
        println!("  Latency: {} ms", latency_ms);
        println!();
        println!(
            "To set as default: {} server set-default {}",
            style("zstash").cyan(),
            &server.id.to_string()[..8]
        );
    }
}

fn print_default_set(output: &OutputMode, server: &ServerInfo) {
    if output.is_json() {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "success": true,
                "server": server
            }))
            .unwrap()
        );
    } else {
        let network_style = match server.network {
            Network::Mainnet => style("mainnet").green(),
            Network::Testnet => style("testnet").yellow(),
        };
        println!(
            "{} '{}' is now the default server for {}",
            style("Done!").green().bold(),
            server.name,
            network_style
        );
    }
}

fn print_test_result(
    output: &OutputMode,
    server: &ServerInfo,
    success: bool,
    latency_ms: Option<u64>,
    error: Option<String>,
) {
    if output.is_json() {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "server_id": server.id,
                "server_name": server.name,
                "success": success,
                "latency_ms": latency_ms,
                "error": error
            }))
            .unwrap()
        );
    } else if success {
        println!(
            "{} Server '{}' is reachable",
            style("[OK]").green().bold(),
            server.name
        );
        if let Some(ms) = latency_ms {
            println!("  Latency: {} ms", ms);
        }
    } else {
        println!(
            "{} Server '{}' test failed",
            style("[FAIL]").red().bold(),
            server.name
        );
        if let Some(err) = error {
            println!("  Error: {}", style(err).red());
        }
        if let Some(ms) = latency_ms {
            println!("  Response time: {} ms", ms);
        }
    }
}
