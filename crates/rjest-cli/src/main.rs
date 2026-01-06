use anyhow::{Context, Result};

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
    if args.daemon_health {
        return daemon::health();
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

    // Handle watch mode
    if args.watch || args.watch_all {
        return run_watch_mode(args);
    }

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

fn run_watch_mode(args: Args) -> Result<()> {
    use rjest_protocol::{
        Request, Response, WatchStartRequest, WatchPollRequest, WatchStopRequest,
    };
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;

    // Set up Ctrl+C handler
    let running = Arc::new(AtomicBool::new(true));
    let r = running.clone();
    ctrlc::set_handler(move || {
        eprintln!("\n\nWatch mode interrupted");
        r.store(false, Ordering::SeqCst);
    })?;

    // Create a pooled client for the watch session (more efficient than creating new socket each poll)
    let mut client = client::Client::new().context("Failed to connect to daemon")?;

    // Build watch start request
    let watch_request = WatchStartRequest {
        project_root: std::env::current_dir()?.to_string_lossy().to_string(),
        patterns: args.patterns.clone(),
        flags: args.to_run_flags(),
    };

    // Start watch session
    eprintln!("\nWatch mode enabled. Press Ctrl+C to exit.\n");
    let response = client.send_request(Request::WatchStart(watch_request))?;

    let session_id = match response {
        Response::WatchStarted(watch_started) => {
            // Render initial results
            output::render(&watch_started.initial_run, &args)?;
            if !watch_started.initial_run.success {
                eprintln!("\nTests failed. Watching for changes...");
            } else {
                eprintln!("\nWatching for changes...");
            }
            watch_started.session_id
        }
        Response::Error(err) => {
            anyhow::bail!("Failed to start watch mode: {}", err.message);
        }
        _ => {
            anyhow::bail!("Unexpected response from daemon");
        }
    };

    // Poll loop - reuses the same connection for better performance
    while running.load(Ordering::SeqCst) {
        let poll_request = WatchPollRequest {
            session_id: session_id.clone(),
            timeout_ms: 1000, // Poll every second
        };

        let response = client.send_request(Request::WatchPoll(poll_request))?;

        match response {
            Response::WatchPoll(poll_response) => {
                if poll_response.has_changes {
                    if !poll_response.changed_files.is_empty() {
                        eprintln!("\n\x1b[2m--- File changes detected ---\x1b[0m");
                        for file in &poll_response.changed_files {
                            eprintln!("  {}", file);
                        }
                    }

                    if let Some(run_result) = poll_response.run_result {
                        eprintln!();
                        output::render(&run_result, &args)?;
                        if run_result.success {
                            eprintln!("\nWatching for changes...");
                        } else {
                            eprintln!("\nTests failed. Watching for changes...");
                        }
                    }
                }
            }
            Response::Error(err) => {
                eprintln!("Watch error: {}", err.message);
            }
            _ => {}
        }
    }

    // Stop watch session
    let stop_request = WatchStopRequest {
        session_id: session_id.clone(),
    };
    let _ = client.send_request(Request::WatchStop(stop_request));

    Ok(())
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
