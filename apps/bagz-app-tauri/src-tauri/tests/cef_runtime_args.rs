#![cfg(all(feature = "cef-runtime", not(feature = "test-bridge")))]

use bagz_app_tauri_lib::{CEF_DISABLED_FEATURES, CEF_HOST_RESOLVER_RULES, cef_runtime_args};
use std::collections::BTreeSet;

const EXPECTED_SWITCHES: &[&str] = &[
    "disable-background-networking",
    "disable-breakpad",
    "disable-component-extensions-with-background-pages",
    "disable-component-update",
    "disable-default-apps",
    "disable-domain-reliability",
    "disable-extensions",
    "disable-field-trial-config",
    "disable-notifications",
    "disable-print-preview",
    "disable-save-password-bubble",
    "disable-speech-api",
    "disable-sync",
    "disable-sync-invalidation-optimizations",
    "incognito",
    "metrics-recording-only",
    "no-default-browser-check",
    "no-first-run",
    "no-pings",
];

const EXPECTED_DISABLED_FEATURES: &[&str] = &[
    "AutofillActorMode",
    "AutofillServerCommunication",
    "AsyncDns",
    "DnsOverHttpsUpgrade",
    "EnableMediaRouter",
    "GlicActorUi",
    "LensOverlay",
    "LiveTranslate",
    "MediaRouter",
    "OptimizationGuideModelExecution",
    "OptimizationGuideOnDeviceModel",
    "OptimizationHints",
    "PrivacySandboxSettings4",
    "Translate",
    "UseDnsHttpsSvcb",
];

const EXPECTED_HOST_RESOLVER_EXCLUDES: &[&str] = &[
    "localhost",
    "127.0.0.1",
    "::1",
    "*.localhost",
    "ipc.localhost",
    "tauri.localhost",
];

fn normalized_key(key: &str) -> &str {
    key.trim_start_matches('-')
}

fn arg_value(args: &[(String, Option<String>)], name: &str) -> Option<Option<String>> {
    let mut iter = args.iter().filter(|(key, _)| normalized_key(key) == name);
    let first = iter.next();
    assert!(
        iter.next().is_none(),
        "CEF switch {name} appears more than once in cef_runtime_args"
    );
    first.map(|(_, value)| value.clone())
}

fn comma_set(raw: &str) -> BTreeSet<&str> {
    raw.split(',')
        .map(str::trim)
        .filter(|entry| !entry.is_empty())
        .collect()
}

#[test]
fn required_switches_present() {
    let args = cef_runtime_args();

    for expected in EXPECTED_SWITCHES {
        assert_eq!(
            arg_value(&args, expected),
            Some(None),
            "missing no-value CEF switch: {expected}"
        );
    }
}

#[test]
fn no_duplicate_switches() {
    let args = cef_runtime_args();
    let mut seen: BTreeSet<String> = BTreeSet::new();
    for (key, _) in &args {
        let normalized = normalized_key(key).to_string();
        assert!(
            seen.insert(normalized.clone()),
            "duplicate CEF switch detected: {normalized}"
        );
    }
}

#[test]
fn disabled_features_exact_set() {
    let args = cef_runtime_args();
    let Some(Some(value)) = arg_value(&args, "disable-features") else {
        panic!("missing --disable-features value");
    };

    let expected = EXPECTED_DISABLED_FEATURES
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();

    assert_eq!(comma_set(CEF_DISABLED_FEATURES), expected);
    assert_eq!(comma_set(&value), expected);
}

#[test]
fn host_resolver_rules_exact() {
    let args = cef_runtime_args();
    let Some(Some(value)) = arg_value(&args, "host-resolver-rules") else {
        panic!("missing --host-resolver-rules value");
    };

    assert_eq!(value, CEF_HOST_RESOLVER_RULES);

    let entries = value
        .split(',')
        .map(str::trim)
        .filter(|entry| !entry.is_empty())
        .collect::<Vec<_>>();

    assert_eq!(entries.first(), Some(&"MAP * 0.0.0.0"));

    let excludes = entries
        .iter()
        .filter_map(|entry| entry.strip_prefix("EXCLUDE "))
        .collect::<BTreeSet<_>>();
    let expected_excludes = EXPECTED_HOST_RESOLVER_EXCLUDES
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();

    assert_eq!(excludes, expected_excludes);
    assert_eq!(entries.len(), expected_excludes.len() + 1);
}

#[test]
fn no_enable_features_switch() {
    let args = cef_runtime_args();

    assert!(
        args.iter()
            .all(|(key, _)| normalized_key(key) != "enable-features"),
        "--enable-features must not be present"
    );
}

#[test]
fn dns_over_https_off() {
    let args = cef_runtime_args();

    assert_eq!(
        arg_value(&args, "dns-over-https-mode"),
        Some(Some("off".to_string()))
    );
    assert_eq!(
        arg_value(&args, "dns-over-https-templates"),
        Some(Some(String::new()))
    );
}

#[test]
fn webrtc_non_proxied_udp_disabled() {
    let args = cef_runtime_args();

    assert_eq!(
        arg_value(&args, "webrtc-ip-handling-policy"),
        Some(Some("disable_non_proxied_udp".to_string()))
    );
}

#[test]
fn no_proxy_or_remote_debugging() {
    let args = cef_runtime_args();
    let forbidden = BTreeSet::from([
        "proxy-server",
        "proxy-pac-url",
        "remote-debugging-port",
        "remote-debugging-pipe",
    ]);

    for (key, _) in args {
        assert!(
            !forbidden.contains(normalized_key(&key)),
            "forbidden CEF switch present: {key}"
        );
    }
}
