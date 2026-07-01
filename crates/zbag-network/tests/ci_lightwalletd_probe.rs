use std::time::Duration;

use zbag_network::grpc_client::GrpcClient;

#[test]
fn ci_probe_lightwalletd_endpoint() {
    if std::env::var_os("CI").is_none() {
        return;
    }

    let endpoint = std::env::var("ZBAG_GRPC_URL")
        .expect("ZBAG_GRPC_URL must be set in CI (see .github/workflows/ci.yml)");

    let client = GrpcClient::new(endpoint.clone());

    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("build tokio runtime");

    let info = rt.block_on(async {
        let mut last_err: Option<anyhow::Error> = None;
        for attempt in 1..=3u32 {
            match client.probe_server().await {
                Ok(info) => return Ok(info),
                Err(err) => {
                    last_err = Some(err);
                    tokio::time::sleep(Duration::from_secs(u64::from(attempt))).await;
                }
            }
        }

        Err(last_err.expect("probe failed without an error"))
    });

    let info = info.unwrap_or_else(|e| panic!("lightwalletd probe failed for {endpoint}: {e:#}"));
    assert!(
        !info.chain_name.is_empty(),
        "lightwalletd probe returned empty chain_name for {endpoint}"
    );
}
