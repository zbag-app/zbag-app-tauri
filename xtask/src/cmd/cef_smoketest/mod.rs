use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::cli::CefSmoketestArgs;

pub mod bundle;
pub mod exit;
pub mod log;
pub mod lsof;
pub mod parser;
pub mod process;
pub mod sampler;
pub mod selftest;
pub mod smoke;

use exit::ExitCode;
use log::LogArtifact;

pub fn run(args: CefSmoketestArgs) -> ExitCode {
    let log = match LogArtifact::new(args.log_path.clone()) {
        Ok(log) => Arc::new(log),
        Err(err) => {
            eprintln!("error: failed to initialize CEF smoke log: {err}");
            return ExitCode::Instrument;
        }
    };

    let signal_requested = Arc::new(AtomicBool::new(false));
    let signal_flag = Arc::clone(&signal_requested);
    if let Err(err) = ctrlc::set_handler(move || {
        signal_flag.store(true, Ordering::SeqCst);
    }) {
        log.write(&format!("error: failed to install signal handler: {err}"));
        return ExitCode::Instrument;
    }

    let mut exit = if args.selftest {
        selftest::run(&log)
    } else {
        smoke::run_smoke(&args, &log, Arc::clone(&signal_requested))
    };

    if signal_requested.load(Ordering::SeqCst) {
        log.write("FAIL: signal received during CEF network smoke");
        exit = ExitCode::Instrument;
    }

    exit
}
