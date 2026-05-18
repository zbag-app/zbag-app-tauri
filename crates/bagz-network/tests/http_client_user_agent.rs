use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::mpsc;

use bagz_network::http_client::HttpClient;

#[tokio::test]
async fn direct_requests_include_user_agent_header() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let addr = listener.local_addr().expect("local addr");

    let (tx, rx) = mpsc::channel::<bool>();

    std::thread::spawn(move || {
        let (mut stream, _peer) = listener.accept().expect("accept");

        let mut buf = Vec::new();
        let mut tmp = [0u8; 1024];
        loop {
            let n = stream.read(&mut tmp).expect("read");
            if n == 0 {
                break;
            }
            buf.extend_from_slice(&tmp[..n]);
            if buf.windows(4).any(|w| w == b"\r\n\r\n") {
                break;
            }
        }

        let req = String::from_utf8_lossy(&buf);
        let has_user_agent = req
            .lines()
            .any(|line| line.to_ascii_lowercase().starts_with("user-agent:") && line.len() > 11);

        let _ = tx.send(has_user_agent);

        let body = b"{\"ok\":true}";
        let resp = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n",
            body.len()
        );
        stream.write_all(resp.as_bytes()).expect("write headers");
        stream.write_all(body).expect("write body");
    });

    let client = HttpClient::new().expect("client");
    let url = reqwest::Url::parse(&format!("http://{}/", addr)).expect("url");
    let res = client.get_json(url).await.expect("request");
    assert_eq!(res.status, 200);
    assert!(rx.recv().expect("user-agent flag"));
}
