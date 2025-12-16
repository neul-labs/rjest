use anyhow::Result;

mod args;
mod client;
mod daemon;
mod output;

use args::Args;
use clap::Parser;

fn main() -> Result<()> {
    let args = Args::parse();

    // Handle daemon management commands
    if args.daemon_status {
        return daemon::status();
    }
    if args.daemon_stop {
        return daemon::stop();
    }
    if args.daemon_restart {
        daemon::stop().ok();
        return run_tests(args);
    }

    // Handle fallback to upstream Jest
    if args.fallback_to_jest || std::env::var("RJEST_FALLBACK").is_ok() {
        return fallback::run_upstream_jest(&args);
    }

    run_tests(args)
}

fn run_tests(args: Args) -> Result<()> {
    // Ensure daemon is running
    daemon::ensure_running()?;

    // Build request from args
    let request = args.to_run_request()?;

    // Send to daemon and get response
    let response = client::send_request(rjest_protocol::Request::Run(request))?;

    // Render output
    match response {
        rjest_protocol::Response::Run(run_response) => {
            output::render(&run_response, &args)?;
            if run_response.success {
                Ok(())
            } else {
                std::process::exit(1);
            }
        }
        rjest_protocol::Response::Error(err) => {
            anyhow::bail!("{}: {}", err.code as u8, err.message);
        }
        _ => {
            anyhow::bail!("Unexpected response from daemon");
        }
    }
}

mod fallback {
    use super::*;
    use std::process::Command;

    pub fn run_upstream_jest(args: &Args) -> Result<()> {
        eprintln!("rjest: falling back to upstream Jest");

        let mut cmd = Command::new("npx");
        cmd.arg("jest");

        // Forward patterns
        for pattern in &args.patterns {
            cmd.arg(pattern);
        }

        // Forward flags
        if args.run_in_band {
            cmd.arg("--runInBand");
        }
        if args.watch {
            cmd.arg("--watch");
        }
        if args.watch_all {
            cmd.arg("--watchAll");
        }
        if args.bail {
            cmd.arg("--bail");
        }
        if args.json {
            cmd.arg("--json");
        }
        if args.verbose {
            cmd.arg("--verbose");
        }
        if args.coverage {
            cmd.arg("--coverage");
        }
        if args.update_snapshots {
            cmd.arg("-u");
        }
        if args.only_changed {
            cmd.arg("--onlyChanged");
        }
        if let Some(ref max_workers) = args.max_workers {
            cmd.arg(format!("--maxWorkers={}", max_workers));
        }
        if let Some(ref config) = args.config {
            cmd.arg(format!("--config={}", config));
        }
        if let Some(ref pattern) = args.test_name_pattern {
            cmd.arg(format!("--testNamePattern={}", pattern));
        }
        for file in &args.find_related_tests {
            cmd.arg("--findRelatedTests").arg(file);
        }

        let status = cmd.status()?;
        std::process::exit(status.code().unwrap_or(1));
    }
}
