use std::io::{Read as _, Write as _};
use std::net::TcpListener;
use std::thread;

use zbag_network::near_intents::{NearIntentsClient, QuoteRequest};

fn spawn_mock_1click_quote_retry_server(
    first_message: &'static str,
) -> (String, thread::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind mock server");
    let addr = listener.local_addr().expect("server addr");
    let base_url = format!("http://{addr}");
    let first_message = first_message.to_string();

    let handle = thread::spawn(move || {
        // First request: 400 Failed to get quote
        {
            let (mut stream, _) = listener.accept().expect("accept");
            let mut buf = [0u8; 16 * 1024];
            let n = stream.read(&mut buf).expect("read request");
            let req = String::from_utf8_lossy(&buf[..n]);

            let body = req.split("\r\n\r\n").nth(1).unwrap_or_default();
            let json: serde_json::Value =
                serde_json::from_str(body).expect("parse request json (attempt 1)");
            assert_eq!(
                json.get("quoteWaitingTimeMs")
                    .and_then(|v| v.as_u64())
                    .unwrap_or_default(),
                3000,
                "initial quoteWaitingTimeMs should be 3000"
            );

            let err_body = format!(
                "{{\"message\":\"{}\",\"correlationId\":\"test-correlation-id-1\",\"timestamp\":\"2026-01-09T15:00:00.000Z\",\"path\":\"/v0/quote\"}}",
                first_message
            );
            let response = format!(
                "HTTP/1.1 400 Bad Request\r\nContent-Type: application/json\r\nConnection: close\r\nContent-Length: {}\r\n\r\n{}",
                err_body.len(),
                err_body
            );
            stream
                .write_all(response.as_bytes())
                .expect("write response");
        }

        // Second request: expect the client to bump quoteWaitingTimeMs and succeed.
        {
            let (mut stream, _) = listener.accept().expect("accept");
            let mut buf = [0u8; 16 * 1024];
            let n = stream.read(&mut buf).expect("read request");
            let req = String::from_utf8_lossy(&buf[..n]);

            let body = req.split("\r\n\r\n").nth(1).unwrap_or_default();
            let json: serde_json::Value =
                serde_json::from_str(body).expect("parse request json (attempt 2)");
            assert_eq!(
                json.get("quoteWaitingTimeMs")
                    .and_then(|v| v.as_u64())
                    .unwrap_or_default(),
                10_000,
                "retry quoteWaitingTimeMs should be 10000"
            );

            let ok_body = r#"{
                "quote": {
                    "amountIn": "1000000000000000",
                    "amountInFormatted": "0.001",
                    "amountInUsd": "3.10",
                    "minAmountIn": "1000000000000000",
                    "amountOut": "672703",
                    "amountOutFormatted": "0.00672703",
                    "amountOutUsd": "2.88",
                    "minAmountOut": "665975",
                    "deadline": "2026-01-10T00:00:00Z",
                    "timeWhenInactive": "2026-01-10T00:00:00Z",
                    "timeEstimate": 160,
                    "depositAddress": "0x0c79D7017D764b3109CEEFF082f3ea6d7b95e8ac",
                    "depositMemo": null
                },
                "quoteRequest": {},
                "signature": "mock",
                "timestamp": "2026-01-09T15:00:00.000Z",
                "correlationId": "test-correlation-id-2"
            }"#;

            let response = format!(
                "HTTP/1.1 201 Created\r\nContent-Type: application/json\r\nConnection: close\r\nContent-Length: {}\r\n\r\n{}",
                ok_body.len(),
                ok_body
            );
            stream
                .write_all(response.as_bytes())
                .expect("write response");
        }
    });

    (base_url, handle)
}

#[tokio::test]
async fn get_quote_retries_failed_to_get_quote_with_longer_wait() {
    let (base_url, server) = spawn_mock_1click_quote_retry_server("Failed to get quote");

    let client = NearIntentsClient::with_base_url(base_url).expect("client");
    let req = QuoteRequest {
        origin_asset: "nep141:eth.omft.near".to_string(),
        destination_asset: "nep141:zec.omft.near".to_string(),
        amount: "1000000000000000".to_string(),
        swap_type: "EXACT_INPUT".to_string(),
        slippage_tolerance: 100,
        quote_waiting_time_ms: Some(3000),
        referral: Some("zbag".to_string()),
        app_fees: None,
        deposit_type: "ORIGIN_CHAIN".to_string(),
        refund_to: "0x3350Fe9Fc38cBa6518471693d748f3f3073C8fdB".to_string(),
        refund_type: "ORIGIN_CHAIN".to_string(),
        recipient: "t1ZMK188cmsdQxYPQi7Y917332HwvsKCdjM".to_string(),
        recipient_type: "DESTINATION_CHAIN".to_string(),
        deadline: "2026-01-09T17:00:00Z".to_string(),
        dry: false,
    };

    let quote = client.get_quote(req).await.expect("quote");
    assert_eq!(quote.amount_in, "1000000000000000");
    assert_eq!(quote.amount_out, "672703");
    assert_eq!(quote.correlation_id, "test-correlation-id-2");

    server.join().expect("server joined");
}

#[tokio::test]
async fn get_quote_retries_no_liquidity_available_with_longer_wait() {
    let (base_url, server) = spawn_mock_1click_quote_retry_server("No liquidity available");

    let client = NearIntentsClient::with_base_url(base_url).expect("client");
    let req = QuoteRequest {
        origin_asset: "nep141:eth.omft.near".to_string(),
        destination_asset: "nep141:zec.omft.near".to_string(),
        amount: "1000000000000000".to_string(),
        swap_type: "EXACT_INPUT".to_string(),
        slippage_tolerance: 100,
        quote_waiting_time_ms: Some(3000),
        referral: Some("zbag".to_string()),
        app_fees: None,
        deposit_type: "ORIGIN_CHAIN".to_string(),
        refund_to: "0x3350Fe9Fc38cBa6518471693d748f3f3073C8fdB".to_string(),
        refund_type: "ORIGIN_CHAIN".to_string(),
        recipient: "t1ZMK188cmsdQxYPQi7Y917332HwvsKCdjM".to_string(),
        recipient_type: "DESTINATION_CHAIN".to_string(),
        deadline: "2026-01-09T17:00:00Z".to_string(),
        dry: false,
    };

    let quote = client.get_quote(req).await.expect("quote");
    assert_eq!(quote.amount_in, "1000000000000000");
    assert_eq!(quote.amount_out, "672703");
    assert_eq!(quote.correlation_id, "test-correlation-id-2");

    server.join().expect("server joined");
}
