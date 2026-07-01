use url::Host;
use url::Url;

use zbag_core::errors;

use crate::error::ipc_err;

/// Validation policy for gRPC endpoint URLs.
///
/// This is an explicit enum (instead of relying solely on `cfg!(debug_assertions)`) so tests can
/// deterministically exercise both policies regardless of how the test binary is compiled.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
enum GrpcUrlPolicy {
    Release,
    Debug,
}

impl GrpcUrlPolicy {
    const fn from_build() -> Self {
        if cfg!(debug_assertions) {
            Self::Debug
        } else {
            Self::Release
        }
    }
}

/// Validate a gRPC endpoint URL.
///
/// Rules:
/// - Must be an absolute URL with a host.
/// - Release builds: only `https://`.
/// - Debug builds: `https://`, plus `http://` for localhost/loopback only.
///   - `localhost` is treated as loopback by name for developer convenience. This assumes typical
///     OS resolver configuration; in unusual environments it may resolve unexpectedly, so don't
///     rely on it for security. Prefer explicit loopback IPs for OS-independent guarantees.
/// - `.onion` addresses are not special-cased; release builds still require `https://`
///   (intentionally strict; see ZbagApp/zbag#103).
///
/// # Errors
///
/// Returns an [`anyhow::Error`] wrapping an IPC error if validation fails.
#[must_use = "validation result must be checked"]
pub fn validate_grpc_url(grpc_url: &str) -> anyhow::Result<()> {
    validate_grpc_url_with_policy(grpc_url, GrpcUrlPolicy::from_build())
}

fn validate_grpc_url_with_policy(grpc_url: &str, policy: GrpcUrlPolicy) -> anyhow::Result<()> {
    // Trim whitespace for consistent behavior across all call sites.
    let grpc_url = grpc_url.trim();

    // `Url::parse` accepts ambiguous inputs like `https:example.com` and `https:///no-host`.
    // For gRPC endpoints we require an explicit authority separator (`://`) and a non-empty host.
    let after_scheme = grpc_url
        .split_once("://")
        .ok_or_else(|| {
            ipc_err(
                errors::INVALID_REQUEST,
                "invalid gRPC URL: expected absolute URL like https://example.com",
            )
        })?
        .1;
    if after_scheme.is_empty() || after_scheme.starts_with('/') {
        return Err(ipc_err(
            errors::INVALID_REQUEST,
            "invalid gRPC URL: missing host",
        ));
    }

    let parsed = Url::parse(grpc_url).map_err(|e| match e {
        url::ParseError::EmptyHost => {
            ipc_err(errors::INVALID_REQUEST, "invalid gRPC URL: missing host")
        }
        url::ParseError::RelativeUrlWithoutBase => ipc_err(
            errors::INVALID_REQUEST,
            "invalid gRPC URL: expected absolute URL like https://example.com",
        ),
        other => ipc_err(
            errors::INVALID_REQUEST,
            format!("invalid gRPC URL: {other}"),
        ),
    })?;

    if !parsed.username().is_empty() || parsed.password().is_some() {
        return Err(ipc_err(
            errors::INVALID_REQUEST,
            "invalid gRPC URL: username/password not allowed",
        ));
    }

    if parsed.query().is_some() {
        return Err(ipc_err(
            errors::INVALID_REQUEST,
            "invalid gRPC URL: query parameters not allowed",
        ));
    }

    if parsed.fragment().is_some() {
        return Err(ipc_err(
            errors::INVALID_REQUEST,
            "invalid gRPC URL: fragments not allowed",
        ));
    }

    if parsed.port() == Some(0) {
        return Err(ipc_err(
            errors::INVALID_REQUEST,
            "invalid gRPC URL: port 0 is not allowed",
        ));
    }

    let scheme = parsed.scheme();

    match policy {
        GrpcUrlPolicy::Release => {
            if scheme != "https" {
                return Err(ipc_err(
                    errors::INVALID_REQUEST,
                    "invalid gRPC URL: only HTTPS URLs are allowed for gRPC servers",
                ));
            }
        }
        GrpcUrlPolicy::Debug => {
            if scheme != "https" {
                let is_localhost = match parsed.host() {
                    // Treat `localhost` as loopback for development. This assumes typical OS
                    // resolver configuration; in unusual environments it may resolve to a
                    // non-loopback address, so don't rely on this for security. Prefer explicit
                    // loopback IP literals for OS-independent behavior.
                    Some(Host::Domain(d)) => d.eq_ignore_ascii_case("localhost"),
                    Some(Host::Ipv4(ip)) => ip.is_loopback(),
                    Some(Host::Ipv6(ip)) => {
                        // Accept IPv4-mapped (`::ffff:127.0.0.1`) IPv6 addresses in debug builds,
                        // but only treat them as loopback when the derived IPv4 address is loopback.
                        ip.is_loopback() || ip.to_ipv4_mapped().is_some_and(|v4| v4.is_loopback())
                    }
                    None => false,
                };

                if scheme != "http" || !is_localhost {
                    return Err(ipc_err(
                        errors::INVALID_REQUEST,
                        "invalid gRPC URL: only HTTPS URLs (or HTTP loopback URLs for development) are allowed",
                    ));
                }
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_grpc_url_accepts_https() {
        assert!(validate_grpc_url("https://lwd.zec.pro").is_ok());
        assert!(validate_grpc_url("https://lwd.zec.pro:443").is_ok());
        assert!(validate_grpc_url("https://lwd.zec.pro/some/path").is_ok());
        assert!(validate_grpc_url("https://example.com").is_ok());
        assert!(validate_grpc_url("https://example.com/").is_ok());
    }

    #[test]
    fn validate_grpc_url_rejects_malformed() {
        assert!(validate_grpc_url("").is_err());
        assert!(validate_grpc_url("not-a-url").is_err());
        assert!(validate_grpc_url("https:example.com").is_err());
    }

    #[test]
    fn validate_grpc_url_rejects_missing_host() {
        assert!(validate_grpc_url("https://").is_err());
        assert!(validate_grpc_url("https:///").is_err());
        assert!(validate_grpc_url("https:///no-host").is_err());
    }

    #[test]
    fn validate_grpc_url_rejects_userinfo() {
        assert!(validate_grpc_url("https://user:pass@example.com").is_err());
        assert!(validate_grpc_url("https://user@example.com").is_err());
    }

    #[test]
    fn validate_grpc_url_rejects_ipv6_zone_identifiers() {
        // Zone identifiers (e.g., `%eth0`) are rejected by `url::Url::parse`,
        // not by explicit validation in this module.
        for url in [
            "https://[fe80::1%25eth0]:8232",
            "http://[fe80::1%25eth0]:8232",
        ] {
            assert!(validate_grpc_url_with_policy(url, GrpcUrlPolicy::Debug).is_err());
            assert!(validate_grpc_url_with_policy(url, GrpcUrlPolicy::Release).is_err());
        }
    }

    #[test]
    fn validate_grpc_url_respects_build_policy() {
        // `validate_grpc_url` picks its policy based on `cfg!(debug_assertions)`.
        let result = validate_grpc_url("http://localhost:8232");
        if cfg!(debug_assertions) {
            assert!(result.is_ok());
        } else {
            assert!(result.is_err());
        }
    }

    #[test]
    fn validate_grpc_url_policy_allows_http_localhost_in_debug() {
        assert!(
            validate_grpc_url_with_policy("http://localhost:8232", GrpcUrlPolicy::Debug).is_ok()
        );
        assert!(
            validate_grpc_url_with_policy("http://LOCALHOST:8232", GrpcUrlPolicy::Debug).is_ok()
        );
        assert!(
            validate_grpc_url_with_policy("http://LocalHost:8232", GrpcUrlPolicy::Debug).is_ok()
        );
        assert!(
            validate_grpc_url_with_policy("http://127.0.0.1:8232", GrpcUrlPolicy::Debug).is_ok()
        );
        // `Ipv4Addr::is_loopback` matches the entire `127.0.0.0/8` block (not just 127.0.0.1).
        assert!(
            validate_grpc_url_with_policy("http://127.0.0.2:8232", GrpcUrlPolicy::Debug).is_ok()
        );
        assert!(
            validate_grpc_url_with_policy("http://[::ffff:127.0.0.1]:8232", GrpcUrlPolicy::Debug)
                .is_ok()
        );
        assert!(validate_grpc_url_with_policy("http://[::1]:8232", GrpcUrlPolicy::Debug).is_ok());
        assert!(
            validate_grpc_url_with_policy("http://localhost:8232", GrpcUrlPolicy::Release).is_err()
        );
    }

    #[test]
    fn validate_grpc_url_policy_rejects_http_non_loopback_ipv4_mapped_ipv6() {
        assert!(
            validate_grpc_url_with_policy("http://[::ffff:8.8.8.8]:8232", GrpcUrlPolicy::Debug)
                .is_err()
        );
    }

    #[test]
    fn validate_grpc_url_policy_rejects_http_non_localhost() {
        assert!(validate_grpc_url_with_policy("http://lwd.zec.pro", GrpcUrlPolicy::Debug).is_err());
        assert!(
            validate_grpc_url_with_policy("http://example.com:8232", GrpcUrlPolicy::Debug).is_err()
        );
        assert!(
            validate_grpc_url_with_policy("http://lwd.zec.pro", GrpcUrlPolicy::Release).is_err()
        );
        assert!(
            validate_grpc_url_with_policy("http://example.com:8232", GrpcUrlPolicy::Release)
                .is_err()
        );
    }

    #[test]
    fn validate_grpc_url_policy_rejects_non_http_schemes() {
        for url in [
            "ftp://example.com",
            "grpc://example.com",
            "grpc://localhost:8232",
            "ws://example.com",
            "wss://example.com",
            "file://example.com/",
        ] {
            assert!(validate_grpc_url_with_policy(url, GrpcUrlPolicy::Debug).is_err());
            assert!(validate_grpc_url_with_policy(url, GrpcUrlPolicy::Release).is_err());
        }
    }

    #[test]
    fn validate_grpc_url_policy_rejects_http_onion_addresses() {
        assert!(
            validate_grpc_url_with_policy("http://example.onion:8232", GrpcUrlPolicy::Debug)
                .is_err()
        );
        assert!(
            validate_grpc_url_with_policy("http://example.onion:8232", GrpcUrlPolicy::Release)
                .is_err()
        );
    }

    #[test]
    fn validate_grpc_url_policy_accepts_https_onion_addresses() {
        assert!(
            validate_grpc_url_with_policy("https://example.onion:443", GrpcUrlPolicy::Debug)
                .is_ok()
        );
        assert!(
            validate_grpc_url_with_policy("https://example.onion:443", GrpcUrlPolicy::Release)
                .is_ok()
        );
    }

    #[test]
    fn validate_grpc_url_rejects_fragments_and_query_params() {
        assert!(validate_grpc_url("https://example.com#anchor").is_err());
        assert!(validate_grpc_url("https://example.com?key=value").is_err());
        assert!(validate_grpc_url("https://example.com?a=1&b=2#section").is_err());
    }

    #[test]
    fn validate_grpc_url_rejects_port_zero() {
        assert!(validate_grpc_url("https://example.com:0").is_err());
    }
}
