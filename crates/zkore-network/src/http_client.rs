use std::time::Duration;

use bytes::Buf as _;
use bytes::Bytes;
use http::Uri;
use http_body_util::BodyExt as _;
use http_body_util::Full;
use serde::Serialize;
use thiserror::Error;

use crate::transport::{SelectedTransport, TransportConfig, TransportError, TransportSelector};

#[derive(Debug, Clone)]
pub struct JsonResponse {
    pub status: u16,
    pub body: serde_json::Value,
    pub retry_after: Option<Duration>,
}

#[derive(Clone)]
pub struct HttpClient {
    direct: reqwest::Client,
    transport: TransportSelector,
}

impl HttpClient {
    pub fn new() -> anyhow::Result<Self> {
        Self::new_with_transport(TransportSelector::new(TransportConfig::default()))
    }

    pub fn new_with_tor(tor: std::sync::Arc<zkore_tor::TorManager>) -> anyhow::Result<Self> {
        Self::new_with_transport(TransportSelector::with_tor(TransportConfig::default(), tor))
    }

    pub fn new_with_transport(transport: TransportSelector) -> anyhow::Result<Self> {
        let direct = reqwest::Client::builder()
            .timeout(transport.config().timeout)
            .build()?;
        Ok(Self { direct, transport })
    }

    pub async fn get_json(&self, url: reqwest::Url) -> Result<JsonResponse, HttpClientError> {
        match self.transport.select()? {
            SelectedTransport::Direct => {
                let res = self.direct.get(url).send().await?;
                let retry_after = parse_retry_after(&res);
                let status = res.status().as_u16();
                let body = res.json::<serde_json::Value>().await?;
                Ok(JsonResponse {
                    status,
                    body,
                    retry_after,
                })
            }
            SelectedTransport::Tor { client } => {
                let uri: Uri = url
                    .as_str()
                    .parse::<Uri>()
                    .map_err(|e| HttpClientError::InvalidUrl(e.to_string()))?;

                let res = tokio::time::timeout(
                    self.transport.config().timeout,
                    client.http_get_json::<serde_json::Value>(uri, 0, |_| None),
                )
                .await
                .map_err(|_| HttpClientError::Timeout)?
                .map_err(|e| HttpClientError::TorTransport(e.to_string()))?;

                let retry_after = parse_retry_after_headers(res.headers());
                let status = res.status().as_u16();
                let body = res.into_body();
                Ok(JsonResponse {
                    status,
                    body,
                    retry_after,
                })
            }
        }
    }

    pub async fn post_json<T: Serialize + ?Sized>(
        &self,
        url: reqwest::Url,
        payload: &T,
    ) -> Result<JsonResponse, HttpClientError> {
        match self.transport.select()? {
            SelectedTransport::Direct => {
                let res = self.direct.post(url).json(payload).send().await?;
                let retry_after = parse_retry_after(&res);
                let status = res.status().as_u16();
                let body = res.json::<serde_json::Value>().await?;
                Ok(JsonResponse {
                    status,
                    body,
                    retry_after,
                })
            }
            SelectedTransport::Tor { client } => {
                let uri: Uri = url
                    .as_str()
                    .parse::<Uri>()
                    .map_err(|e| HttpClientError::InvalidUrl(e.to_string()))?;

                let body_bytes = serde_json::to_vec(payload)
                    .map_err(|e| HttpClientError::InvalidBody(e.to_string()))?;

                let body = Full::new(Bytes::from(body_bytes));
                let res = tokio::time::timeout(
                    self.transport.config().timeout,
                    client.http_post(
                        uri,
                        |builder| {
                            builder
                                .header(http::header::ACCEPT, "application/json")
                                .header(http::header::CONTENT_TYPE, "application/json")
                        },
                        body,
                        |body| async move {
                            let aggregated = body
                                .collect()
                                .await
                                .map_err(zcash_client_backend::tor::http::HttpError::from)?
                                .aggregate();
                            let value = serde_json::from_reader(aggregated.reader())
                                .map_err(zcash_client_backend::tor::http::HttpError::from)?;
                            Ok(value)
                        },
                        0,
                        |_| None,
                    ),
                )
                .await
                .map_err(|_| HttpClientError::Timeout)?
                .map_err(|e| HttpClientError::TorTransport(e.to_string()))?;

                let retry_after = parse_retry_after_headers(res.headers());
                let status = res.status().as_u16();
                let body = res.into_body();
                Ok(JsonResponse {
                    status,
                    body,
                    retry_after,
                })
            }
        }
    }
}

#[derive(Debug, Error)]
pub enum HttpClientError {
    #[error(transparent)]
    DirectTransport(#[from] reqwest::Error),
    #[error(transparent)]
    FailClosed(#[from] TransportError),
    #[error("Tor transport error: {0}")]
    TorTransport(String),
    #[error("Tor request timed out")]
    Timeout,
    #[error("invalid url: {0}")]
    InvalidUrl(String),
    #[error("invalid body: {0}")]
    InvalidBody(String),
}

fn parse_retry_after(res: &reqwest::Response) -> Option<Duration> {
    res.headers()
        .get("retry-after")
        .and_then(|h| h.to_str().ok())
        .and_then(|s| s.parse::<u64>().ok())
        .map(Duration::from_secs)
}

fn parse_retry_after_headers(headers: &http::HeaderMap) -> Option<Duration> {
    headers
        .get("retry-after")
        .and_then(|h| h.to_str().ok())
        .and_then(|s| s.parse::<u64>().ok())
        .map(Duration::from_secs)
}
