// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

use clap::*;
use colored::Colorize;
use sui::client_commands::SuiClientCommands::ReplayTransaction;
use sui::sui_commands::SuiCommand;
use sui_types::exit_main;
use tracing::debug;

const GIT_REVISION: &str = {
    if let Some(revision) = option_env!("GIT_REVISION") {
        revision
    } else {
        let version = git_version::git_version!(
            args = ["--always", "--dirty", "--exclude", "*"],
            fallback = ""
        );
        version
    }
};

const VERSION: &str = {
    if GIT_REVISION.is_empty() {
        env!("CARGO_PKG_VERSION")
    } else {
        const_str::concat!(env!("CARGO_PKG_VERSION"), "-", GIT_REVISION)
    }
};

#[derive(Parser)]
#[clap(
    name = env!("CARGO_BIN_NAME"),
    about = "A Byzantine fault tolerant chain with low-latency finality and high throughput",
    rename_all = "kebab-case",
    author,
    version = VERSION,
    propagate_version = true,
)]
struct Args {
    #[clap(subcommand)]
    command: SuiCommand,
}

#[tokio::main]
async fn main() {
    #[cfg(windows)]
    colored::control::set_virtual_terminal(true).unwrap();

    let args = Args::parse();
    let _guard = match args.command {
        SuiCommand::Console { .. } | SuiCommand::KeyTool { .. } | SuiCommand::Move { .. } => {
            telemetry_subscribers::TelemetryConfig::new()
                .with_log_level("error")
                .with_env()
                .init()
        }
        SuiCommand::Client {
            cmd: Some(ReplayTransaction { gas_info, .. }),
            ..
        } => {
            if gas_info {
                telemetry_subscribers::TelemetryConfig::new()
                    .with_log_level("error")
                    .with_trace_target("replay")
                    .with_env()
                    .init()
            } else {
                telemetry_subscribers::TelemetryConfig::new()
                    .with_log_level("error")
                    .with_env()
                    .init()
            }
        }
        _ => telemetry_subscribers::TelemetryConfig::new()
            .with_log_level("error")
            .with_env()
            .init(),
    };

    debug!("Sui CLI version: {VERSION}");

    exit_main!(args.command.execute().await);
}
