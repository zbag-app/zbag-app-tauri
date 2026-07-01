use std::num::NonZeroU32;

use clap::Parser;

use zbag_xtask::cli::{CefSmoketestArgs, Cli};
use zbag_xtask::promote_selftest_env;

#[test]
fn clap_parse_errors_exit_two() {
    let err =
        Cli::try_parse_from(["xtask", "cef-smoketest", "--no-such-flag"]).expect_err("parse error");
    assert_eq!(err.exit_code(), 2);
}

#[test]
fn clap_rejects_zero_duration_with_exit_two() {
    let err = Cli::try_parse_from(["xtask", "cef-smoketest", "--duration-secs", "0"])
        .expect_err("parse error");
    assert_eq!(err.exit_code(), 2);
}

#[test]
fn selftest_env_promotes_only_exact_one() {
    let mut parsed_args = test_args(false);
    promote_selftest_env(&mut parsed_args, |_| Some("1".to_string()));
    assert!(parsed_args.selftest);

    for value in ["true", "yes", "0"] {
        let mut parsed_args = test_args(false);
        promote_selftest_env(&mut parsed_args, |_| Some(value.to_string()));
        assert!(!parsed_args.selftest, "{value}");
    }

    let mut parsed_args = test_args(false);
    promote_selftest_env(&mut parsed_args, |_| None);
    assert!(!parsed_args.selftest);
}

#[test]
fn selftest_flag_stays_enabled_without_env() {
    let mut parsed_args = test_args(true);
    promote_selftest_env(&mut parsed_args, |_| None);
    assert!(parsed_args.selftest);
}

fn test_args(selftest: bool) -> CefSmoketestArgs {
    CefSmoketestArgs {
        app: None,
        selftest,
        duration_secs: NonZeroU32::new(15).expect("non-zero"),
        lsof_timeout_secs: NonZeroU32::new(3).expect("non-zero"),
        log_path: None,
    }
}
