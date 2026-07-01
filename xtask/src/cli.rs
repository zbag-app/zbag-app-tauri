use std::num::NonZeroU32;
use std::path::PathBuf;

use clap::{Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(name = "xtask", about = "zbag developer tools")]
pub struct Cli {
    #[command(subcommand)]
    pub cmd: Cmd,
}

#[derive(Debug, Subcommand)]
pub enum Cmd {
    CefSmoketest(CefSmoketestArgs),
}

#[derive(Clone, Debug, clap::Args)]
pub struct CefSmoketestArgs {
    #[arg(long, value_name = "PATH")]
    pub app: Option<PathBuf>,

    #[arg(long)]
    pub selftest: bool,

    #[arg(long, env = "ZBAG_SMOKE_DURATION_SECS", default_value = "15")]
    pub duration_secs: NonZeroU32,

    #[arg(long, env = "ZBAG_LSOF_TIMEOUT_SECS", default_value = "3")]
    pub lsof_timeout_secs: NonZeroU32,

    #[arg(long, value_name = "PATH")]
    pub log_path: Option<PathBuf>,
}
