//! Lightwalletd server management commands.

use std::collections::BTreeSet;
use std::path::Path;
use std::time::{Duration, Instant};

use anyhow::{Context as _, Result};
use clap::{Args, Subcommand};
use console::style;
use prost::Message as _;
use serde::Serialize;
use tokio::task::JoinSet;
use uuid::Uuid;
use zcash_protocol::consensus::BlockHeight;

use bagz_core::domain::{Network, ServerInfo};
use bagz_engine::error::find_engine_ipc_error;
use bagz_engine::grpc_url::validate_grpc_url;
use bagz_network::grpc_client::GrpcClient;

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

    /// Benchmark compact block fetch throughput and find the fastest settings
    BenchFetch {
        /// gRPC URL (e.g., https://lwd.mainnet.example.com)
        #[arg(long)]
        url: String,

        /// Inclusive start height (default: end - sample_blocks)
        #[arg(long)]
        start_height: Option<u32>,

        /// Exclusive end height (default: latest tip + 1)
        #[arg(long)]
        end_height: Option<u32>,

        /// Number of blocks to sample when start_height is omitted
        #[arg(long, default_value_t = 20_000)]
        sample_blocks: u32,

        /// Candidate batch sizes (comma-separated)
        #[arg(long, value_delimiter = ',', default_values_t = [100_u32, 250_u32, 500_u32, 1000_u32])]
        batch_sizes: Vec<u32>,

        /// Candidate parallel fetch values (comma-separated)
        #[arg(long, value_delimiter = ',', default_values_t = [1_usize, 2_usize, 4_usize, 8_usize])]
        parallelism: Vec<usize>,

        /// Number of runs per configuration
        #[arg(long, default_value_t = 2)]
        repeats: u32,

        /// Per-batch retry attempts
        #[arg(long, default_value_t = 2)]
        retries: u32,

        /// Per-batch timeout in seconds
        #[arg(long, default_value_t = 25)]
        timeout_secs: u64,
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
        ServerCommand::BenchFetch {
            url,
            start_height,
            end_height,
            sample_blocks,
            batch_sizes,
            parallelism,
            repeats,
            retries,
            timeout_secs,
        } => {
            let config = FetchBenchConfig {
                url,
                start_height,
                end_height,
                sample_blocks,
                batch_sizes,
                parallelism,
                repeats,
                retries,
                timeout_secs,
            };
            benchmark_fetch(config, output).await
        }
    }
}

async fn list_servers(data_dir: &Path, output: &OutputMode) -> Result<()> {
    let state = CliAppState::new(data_dir, false)?;

    let servers = {
        let wm = state.wallet_manager.lock().expect("mutex poisoned");
        bagz_engine::db::server_meta::list_servers(wm.app_db().conn())
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
        bagz_engine::db::server_meta::insert_server(wm.app_db().conn(), &server, now_ms)
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
        bagz_engine::db::server_meta::get_server(wm.app_db().conn(), server_id)
            .map_err(|e| anyhow::anyhow!(e))?
            .ok_or_else(|| anyhow::anyhow!("server not found: {}", server_id_str))?
    };

    // Defense-in-depth: stored values may be tampered with or come from legacy versions.
    validate_grpc_url(&server.grpc_url)?;

    {
        let mut wm = state.wallet_manager.lock().expect("mutex poisoned");
        bagz_engine::db::server_meta::set_default_server(wm.app_db_mut().conn_mut(), server_id)
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
        bagz_engine::db::server_meta::get_server(wm.app_db().conn(), server_id)
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
                let _ = bagz_engine::db::server_meta::update_last_success_at(
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
        bagz_engine::db::server_meta::list_servers(wm.app_db().conn())
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
                style("bagz").cyan()
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
            style("bagz").cyan(),
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

#[derive(Debug, Clone, Serialize)]
struct FetchRunResult {
    run: u32,
    elapsed_ms: u128,
    batches: u32,
    requested_blocks: u64,
    returned_blocks: u64,
    bytes: u64,
    retries_used: u32,
    blocks_per_sec: f64,
    mib_per_sec: f64,
}

#[derive(Debug, Clone, Serialize)]
struct FetchProfileResult {
    batch_size: u32,
    parallelism: usize,
    successful_runs: u32,
    failed_runs: u32,
    avg_blocks_per_sec: f64,
    avg_mib_per_sec: f64,
    avg_elapsed_ms: f64,
    avg_retries_per_run: f64,
    runs: Vec<FetchRunResult>,
    last_error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
struct FetchBestConfig {
    batch_size: u32,
    parallelism: usize,
    avg_blocks_per_sec: f64,
    avg_mib_per_sec: f64,
}

#[derive(Debug, Clone, Serialize)]
struct FetchBenchmarkReport {
    endpoint: String,
    tip_height: u32,
    start_height: u32,
    end_height_exclusive: u32,
    total_blocks: u32,
    repeats: u32,
    retries: u32,
    timeout_secs: u64,
    profiles: Vec<FetchProfileResult>,
    best: Option<FetchBestConfig>,
}

#[derive(Debug, Clone)]
struct BatchFetchResult {
    requested_blocks: u64,
    returned_blocks: u64,
    bytes: u64,
    retries_used: u32,
}

#[derive(Debug, Clone)]
struct FetchBenchConfig {
    url: String,
    start_height: Option<u32>,
    end_height: Option<u32>,
    sample_blocks: u32,
    batch_sizes: Vec<u32>,
    parallelism: Vec<usize>,
    repeats: u32,
    retries: u32,
    timeout_secs: u64,
}

#[derive(Debug, Clone, Copy)]
struct FetchProfileParams {
    start: u32,
    end_exclusive: u32,
    batch_size: u32,
    parallelism: usize,
    retries: u32,
    timeout: Duration,
    run: u32,
}

async fn benchmark_fetch(config: FetchBenchConfig, output: &OutputMode) -> Result<()> {
    let FetchBenchConfig {
        url,
        start_height,
        end_height,
        sample_blocks,
        batch_sizes,
        parallelism,
        repeats,
        retries,
        timeout_secs,
    } = config;

    validate_grpc_url(&url)?;

    if sample_blocks == 0 {
        anyhow::bail!("sample_blocks must be greater than 0");
    }
    if repeats == 0 {
        anyhow::bail!("repeats must be greater than 0");
    }
    if timeout_secs == 0 {
        anyhow::bail!("timeout_secs must be greater than 0");
    }

    let batch_sizes = normalize_u32_candidates(batch_sizes, "batch_sizes")?;
    let parallelism = normalize_usize_candidates(parallelism, "parallelism")?;

    let client = GrpcClient::new(url.clone());
    let (tip_height, _) = client
        .get_latest_block()
        .await
        .context("failed to get chain tip from server")?;
    let tip_u32 = u32::from(tip_height);
    let tip_plus_one = tip_u32.saturating_add(1);

    let mut end_exclusive = end_height.unwrap_or(tip_plus_one);
    if end_exclusive > tip_plus_one {
        end_exclusive = tip_plus_one;
    }

    let start = start_height.unwrap_or_else(|| end_exclusive.saturating_sub(sample_blocks));
    if start >= end_exclusive {
        anyhow::bail!(
            "invalid range: start_height ({}) must be less than end_height ({})",
            start,
            end_exclusive
        );
    }

    if !output.is_json() {
        println!("Benchmarking {}", style(&url).cyan());
        println!(
            "Range: {}..{} ({} blocks, tip {})",
            start,
            end_exclusive,
            end_exclusive - start,
            tip_u32
        );
        println!(
            "Profiles: {} batch sizes x {} parallelism levels x {} runs",
            batch_sizes.len(),
            parallelism.len(),
            repeats
        );
        println!();
    }

    let timeout = Duration::from_secs(timeout_secs);
    let mut profiles = Vec::new();

    for &batch_size in &batch_sizes {
        for &parallel in &parallelism {
            let mut runs = Vec::new();
            let mut failed_runs = 0u32;
            let mut last_error = None;

            if !output.is_json() {
                println!(
                    "Running profile batch_size={} parallelism={} ...",
                    batch_size, parallel
                );
            }

            for run_idx in 1..=repeats {
                let profile_params = FetchProfileParams {
                    start,
                    end_exclusive,
                    batch_size,
                    parallelism: parallel,
                    retries,
                    timeout,
                    run: run_idx,
                };
                match run_fetch_profile(&client, profile_params).await {
                    Ok(run) => {
                        if !output.is_json() {
                            println!(
                                "  run {:>2}: {:>8.2} blocks/s  {:>8.2} MiB/s  retries={}",
                                run.run, run.blocks_per_sec, run.mib_per_sec, run.retries_used
                            );
                        }
                        runs.push(run);
                    }
                    Err(err) => {
                        failed_runs = failed_runs.saturating_add(1);
                        last_error = Some(err.to_string());
                        if !output.is_json() {
                            println!("  run {:>2}: {}", run_idx, style("FAILED").red().bold());
                            println!("           {}", style(err).red());
                        }
                    }
                }
            }

            let successful_runs = runs.len() as u32;
            let avg_blocks_per_sec = average_f64(runs.iter().map(|r| r.blocks_per_sec));
            let avg_mib_per_sec = average_f64(runs.iter().map(|r| r.mib_per_sec));
            let avg_elapsed_ms = average_f64(runs.iter().map(|r| r.elapsed_ms as f64));
            let avg_retries_per_run = average_f64(runs.iter().map(|r| r.retries_used as f64));

            profiles.push(FetchProfileResult {
                batch_size,
                parallelism: parallel,
                successful_runs,
                failed_runs,
                avg_blocks_per_sec,
                avg_mib_per_sec,
                avg_elapsed_ms,
                avg_retries_per_run,
                runs,
                last_error,
            });
        }
    }

    let best = profiles
        .iter()
        .filter(|p| p.successful_runs > 0)
        .max_by(|a, b| {
            a.avg_blocks_per_sec
                .total_cmp(&b.avg_blocks_per_sec)
                .then_with(|| b.failed_runs.cmp(&a.failed_runs))
        })
        .map(|p| FetchBestConfig {
            batch_size: p.batch_size,
            parallelism: p.parallelism,
            avg_blocks_per_sec: p.avg_blocks_per_sec,
            avg_mib_per_sec: p.avg_mib_per_sec,
        });

    let report = FetchBenchmarkReport {
        endpoint: url,
        tip_height: tip_u32,
        start_height: start,
        end_height_exclusive: end_exclusive,
        total_blocks: end_exclusive - start,
        repeats,
        retries,
        timeout_secs,
        profiles,
        best,
    };

    print_benchmark_report(output, &report);
    Ok(())
}

async fn run_fetch_profile(
    client: &GrpcClient,
    params: FetchProfileParams,
) -> Result<FetchRunResult> {
    let FetchProfileParams {
        start,
        end_exclusive,
        batch_size,
        parallelism,
        retries,
        timeout,
        run,
    } = params;

    let mut next_height = start;
    let mut join_set: JoinSet<Result<BatchFetchResult>> = JoinSet::new();
    let mut in_flight = 0usize;
    let mut batches = 0u32;
    let mut requested_blocks = 0u64;
    let mut returned_blocks = 0u64;
    let mut bytes = 0u64;
    let mut retries_used = 0u32;
    let started = Instant::now();

    while next_height < end_exclusive || in_flight > 0 {
        while next_height < end_exclusive && in_flight < parallelism {
            let batch_start = next_height;
            let batch_end = std::cmp::min(batch_start.saturating_add(batch_size), end_exclusive);
            next_height = batch_end;
            batches = batches.saturating_add(1);

            let client = client.clone();
            join_set.spawn(async move {
                fetch_batch_with_retry(client, batch_start, batch_end, retries, timeout).await
            });
            in_flight += 1;
        }

        let Some(joined) = join_set.join_next().await else {
            break;
        };
        in_flight = in_flight.saturating_sub(1);

        let batch = joined.context("fetch worker panicked")??;
        requested_blocks = requested_blocks.saturating_add(batch.requested_blocks);
        returned_blocks = returned_blocks.saturating_add(batch.returned_blocks);
        bytes = bytes.saturating_add(batch.bytes);
        retries_used = retries_used.saturating_add(batch.retries_used);
    }

    let elapsed = started.elapsed();
    let elapsed_secs = elapsed.as_secs_f64().max(f64::EPSILON);
    let blocks_per_sec = returned_blocks as f64 / elapsed_secs;
    let mib_per_sec = (bytes as f64 / (1024.0 * 1024.0)) / elapsed_secs;

    Ok(FetchRunResult {
        run,
        elapsed_ms: elapsed.as_millis(),
        batches,
        requested_blocks,
        returned_blocks,
        bytes,
        retries_used,
        blocks_per_sec,
        mib_per_sec,
    })
}

async fn fetch_batch_with_retry(
    client: GrpcClient,
    start: u32,
    end_exclusive: u32,
    max_retries: u32,
    timeout: Duration,
) -> Result<BatchFetchResult> {
    if end_exclusive <= start {
        return Ok(BatchFetchResult {
            requested_blocks: 0,
            returned_blocks: 0,
            bytes: 0,
            retries_used: 0,
        });
    }

    let mut attempt = 0u32;
    loop {
        let fetch =
            tokio::time::timeout(timeout, fetch_batch_once(&client, start, end_exclusive)).await;

        match fetch {
            Ok(Ok(mut result)) => {
                result.retries_used = attempt;
                return Ok(result);
            }
            Ok(Err(err)) if attempt < max_retries => {
                attempt = attempt.saturating_add(1);
                let backoff_ms = (250_u64 * (1_u64 << attempt.min(6))).min(4000);
                tokio::time::sleep(Duration::from_millis(backoff_ms)).await;
                tracing::debug!(
                    attempt,
                    max_retries,
                    range = format!("{}..{}", start, end_exclusive),
                    error = ?err,
                    "retrying failed batch download"
                );
            }
            Err(_) if attempt < max_retries => {
                attempt = attempt.saturating_add(1);
                let backoff_ms = (250_u64 * (1_u64 << attempt.min(6))).min(4000);
                tokio::time::sleep(Duration::from_millis(backoff_ms)).await;
                tracing::debug!(
                    attempt,
                    max_retries,
                    range = format!("{}..{}", start, end_exclusive),
                    "retrying timed out batch download"
                );
            }
            Ok(Err(err)) => {
                return Err(err).with_context(|| {
                    format!(
                        "batch {}..{} failed after {} attempts",
                        start,
                        end_exclusive,
                        attempt + 1
                    )
                });
            }
            Err(_) => {
                anyhow::bail!(
                    "batch {}..{} timed out after {} attempts (timeout={}s)",
                    start,
                    end_exclusive,
                    attempt + 1,
                    timeout.as_secs()
                );
            }
        }
    }
}

async fn fetch_batch_once(
    client: &GrpcClient,
    start: u32,
    end_exclusive: u32,
) -> Result<BatchFetchResult> {
    let start_height = BlockHeight::from_u32(start);
    let end_inclusive = BlockHeight::from_u32(end_exclusive.saturating_sub(1));
    let mut stream = client
        .get_block_range(start_height, end_inclusive)
        .await
        .with_context(|| format!("GetBlockRange {}..{} RPC failed", start, end_exclusive))?;

    let mut returned_blocks = 0u64;
    let mut bytes = 0u64;
    while let Some(block) = stream
        .message()
        .await
        .map_err(anyhow::Error::from)
        .with_context(|| format!("stream read failed for range {}..{}", start, end_exclusive))?
    {
        returned_blocks = returned_blocks.saturating_add(1);
        bytes = bytes.saturating_add(block.encoded_len() as u64);
    }

    Ok(BatchFetchResult {
        requested_blocks: u64::from(end_exclusive - start),
        returned_blocks,
        bytes,
        retries_used: 0,
    })
}

fn normalize_u32_candidates(values: Vec<u32>, field: &str) -> Result<Vec<u32>> {
    let mut set = BTreeSet::new();
    for value in values {
        if value == 0 {
            anyhow::bail!("{field} entries must be > 0");
        }
        set.insert(value);
    }
    if set.is_empty() {
        anyhow::bail!("{field} must contain at least one value");
    }
    Ok(set.into_iter().collect())
}

fn normalize_usize_candidates(values: Vec<usize>, field: &str) -> Result<Vec<usize>> {
    let mut set = BTreeSet::new();
    for value in values {
        if value == 0 {
            anyhow::bail!("{field} entries must be > 0");
        }
        set.insert(value);
    }
    if set.is_empty() {
        anyhow::bail!("{field} must contain at least one value");
    }
    Ok(set.into_iter().collect())
}

fn average_f64(values: impl Iterator<Item = f64>) -> f64 {
    let mut count = 0u64;
    let mut sum = 0.0f64;
    for value in values {
        sum += value;
        count = count.saturating_add(1);
    }
    if count == 0 { 0.0 } else { sum / count as f64 }
}

fn print_benchmark_report(output: &OutputMode, report: &FetchBenchmarkReport) {
    if output.is_json() {
        println!("{}", serde_json::to_string_pretty(report).unwrap());
        return;
    }

    println!();
    println!("{}", style("Benchmark Results").bold());
    println!(
        "{:<10} {:<11} {:<12} {:<11} {:<11} {:<9} {:<9}",
        "batch", "parallel", "avg blk/s", "avg MiB/s", "avg ms", "ok runs", "fail"
    );
    println!("{}", "-".repeat(78));

    for profile in &report.profiles {
        println!(
            "{:<10} {:<11} {:<12.2} {:<11.2} {:<11.1} {:<9} {:<9}",
            profile.batch_size,
            profile.parallelism,
            profile.avg_blocks_per_sec,
            profile.avg_mib_per_sec,
            profile.avg_elapsed_ms,
            profile.successful_runs,
            profile.failed_runs
        );
        if let Some(err) = &profile.last_error {
            println!("  {} {}", style("last error:").yellow(), err);
        }
    }

    println!();
    match &report.best {
        Some(best) => {
            println!(
                "{} batch_size={} parallelism={} ({:.2} blocks/s, {:.2} MiB/s)",
                style("Best config:").green().bold(),
                best.batch_size,
                best.parallelism,
                best.avg_blocks_per_sec,
                best.avg_mib_per_sec
            );

            if let Some(default_profile) = report
                .profiles
                .iter()
                .find(|p| p.batch_size == 250 && p.parallelism == 1 && p.successful_runs > 0)
            {
                let improvement = if default_profile.avg_blocks_per_sec > 0.0 {
                    ((best.avg_blocks_per_sec / default_profile.avg_blocks_per_sec) - 1.0) * 100.0
                } else {
                    0.0
                };
                println!(
                    "Compared to current sync defaults (batch=250, parallel=1): {:+.2}% blocks/s",
                    improvement
                );
            }
        }
        None => {
            println!(
                "{} no successful profile runs completed",
                style("No winner:").red().bold()
            );
        }
    }
}
