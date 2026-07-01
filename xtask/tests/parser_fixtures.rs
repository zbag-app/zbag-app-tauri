use zbag_xtask::cmd::cef_smoketest::parser::{
    classify_lsof_fields, endpoint_host, fixture_stream, is_loopback_host,
};

#[test]
fn parser_fixture_loopback_listener_passes() {
    assert_fixture("loopback-listener", true);
}

#[test]
fn parser_fixture_wildcard_listener_fails() {
    assert_fixture("wildcard-listener", false);
}

#[test]
fn parser_fixture_zero_listener_fails() {
    assert_fixture("zero-listener", false);
}

#[test]
fn parser_fixture_external_connected_fails() {
    assert_fixture("external-connected", false);
}

#[test]
fn parser_fixture_loopback_connected_passes() {
    assert_fixture("loopback-connected", true);
}

#[test]
fn endpoint_host_handles_ipv4_and_bracketed_ipv6() {
    assert_eq!(endpoint_host("127.0.0.1:7777"), "127.0.0.1");
    assert_eq!(endpoint_host("[::1]:7777"), "::1");
    assert_eq!(endpoint_host("142.250.190.78:443"), "142.250.190.78");
}

#[test]
fn loopback_host_matches_bash_contract() {
    assert!(is_loopback_host("127.0.0.1"));
    assert!(is_loopback_host("[::1]"));
    assert!(!is_loopback_host("0.0.0.0"));
    assert!(!is_loopback_host("*"));
}

fn assert_fixture(name: &str, should_pass: bool) {
    let stream = fixture_stream(name).expect("fixture");
    let violations = classify_lsof_fields(&format!("selftest:{name}"), &stream);
    assert_eq!(violations.is_empty(), should_pass, "{name}");
}
