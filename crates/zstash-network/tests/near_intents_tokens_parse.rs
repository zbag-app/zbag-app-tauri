use std::io::{Read as _, Write as _};
use std::net::TcpListener;
use std::thread;

use zstash_network::near_intents::NearIntentsClient;

fn spawn_mock_1click_tokens_server() -> (String, thread::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind mock server");
    let addr = listener.local_addr().expect("server addr");
    let base_url = format!("http://{addr}");

    let handle = thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept");
        let mut buf = [0u8; 16 * 1024];
        let n = stream.read(&mut buf).expect("read request");
        let req = String::from_utf8_lossy(&buf[..n]);

        assert!(
            req.contains("GET /v0/tokens"),
            "expected GET /v0/tokens request, got: {req}"
        );

        let ok_body = r#"
        [
          {
            "assetId": "nep141:base.omft.near",
            "symbol": "ETH",
            "blockchain": "base",
            "decimals": 18,
            "price": 2000.0,
            "contractAddress": null
          },
          {
            "defuseAssetId": "nep141:zec.omft.near",
            "symbol": "ZEC",
            "blockchain": "zec",
            "decimals": 8,
            "usdPrice": 230.0,
            "icon": "https://example.invalid/zec.png"
          }
        ]
        "#;

        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nConnection: close\r\nContent-Length: {}\r\n\r\n{}",
            ok_body.len(),
            ok_body
        );
        stream
            .write_all(response.as_bytes())
            .expect("write response");
    });

    (base_url, handle)
}

#[tokio::test]
async fn get_supported_tokens_parses_asset_id_and_price_fields() {
    let (base_url, server) = spawn_mock_1click_tokens_server();

    let client = NearIntentsClient::with_base_url(base_url).expect("client");
    let tokens = client.get_supported_tokens().await.expect("tokens");

    assert_eq!(tokens.len(), 2);

    assert_eq!(tokens[0].asset_id, "nep141:base.omft.near");
    assert_eq!(tokens[0].symbol, "ETH");
    assert_eq!(tokens[0].chain, "base");
    assert_eq!(tokens[0].decimals, 18);
    assert_eq!(tokens[0].usd_price, Some(2000.0));

    // Backwards-compat parsing: defuseAssetId + usdPrice.
    assert_eq!(tokens[1].asset_id, "nep141:zec.omft.near");
    assert_eq!(tokens[1].symbol, "ZEC");
    assert_eq!(tokens[1].chain, "zec");
    assert_eq!(tokens[1].decimals, 8);
    assert_eq!(tokens[1].usd_price, Some(230.0));
    assert_eq!(
        tokens[1].icon.as_deref(),
        Some("https://example.invalid/zec.png")
    );

    server.join().expect("server joined");
}
