use clap::Parser;

pub mod cli;
pub mod cmd;

pub fn run() -> std::process::ExitCode {
    let parsed = cli::Cli::parse();
    let exit = match parsed.cmd {
        cli::Cmd::CefSmoketest(mut args) => {
            promote_selftest_env(&mut args, |key| std::env::var(key).ok());
            cmd::cef_smoketest::run(args)
        }
    };

    exit.into()
}

pub fn promote_selftest_env<F>(args: &mut cli::CefSmoketestArgs, get_env: F)
where
    F: Fn(&str) -> Option<String>,
{
    if !args.selftest && get_env("ZBAG_SMOKE_SELFTEST").as_deref() == Some("1") {
        args.selftest = true;
    }
}
