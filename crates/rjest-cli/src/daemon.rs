use anyhow::{Context, Result};
use rjest_protocol::{socket_path, Request, Response};
use std::process::{Command, Stdio};
use std::thread;
use std::time::Duration;

use crate::client;

/// Ensure the daemon is running, starting it if necessary
pub fn ensure_running() -> Result<()> {
    if client::ping()? {
        return Ok(());
    }

    eprintln!("Starting rjest daemon...");
    start()?;

    // Wait for daemon to be ready
    for _ in 0..50 {
        thread::sleep(Duration::from_millis(100));
        if client::ping()? {
            return Ok(());
        }
    }

    anyhow::bail!("Daemon failed to start within timeout")
}

/// Start the daemon as a background process
fn start() -> Result<()> {
    // Find the jestd binary - it should be alongside this binary
    let current_exe = std::env::current_exe()?;
    let bin_dir = current_exe.parent().context("No parent directory")?;
    let jestd_path = bin_dir.join("jestd");

    if !jestd_path.exists() {
        anyhow::bail!(
            "jestd daemon not found at {}. Please ensure it's installed.",
            jestd_path.display()
        );
    }

    // Spawn daemon in background
    Command::new(&jestd_path)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .context("Failed to spawn daemon")?;

    Ok(())
}

/// Get daemon status
pub fn status() -> Result<()> {
    if !client::ping()? {
        println!("Daemon is not running");
        return Ok(());
    }

    let response = client::send_request(Request::Status)?;

    match response {
        Response::Status(status) => {
            println!("rjest daemon v{}", status.version);
            println!("Uptime: {}s", status.uptime_secs);
            println!("Projects tracked: {}", status.projects_count);
            println!();
            println!("Cache:");
            println!("  Transforms: {} ({} bytes)",
                status.cache_stats.transform_count,
                status.cache_stats.transform_size_bytes
            );
            println!("  Graphs: {}", status.cache_stats.graph_count);
            println!("  Hit rate: {:.1}%", status.cache_stats.hit_rate * 100.0);
            println!();
            println!("Workers:");
            println!("  Active: {}", status.worker_stats.active);
            println!("  Idle: {}", status.worker_stats.idle);
            println!("  Total tests run: {}", status.worker_stats.total_tests_run);
        }
        Response::Error(err) => {
            eprintln!("Error getting status: {}", err.message);
        }
        _ => {
            eprintln!("Unexpected response");
        }
    }

    Ok(())
}

/// Stop the daemon
pub fn stop() -> Result<()> {
    if !client::ping()? {
        println!("Daemon is not running");
        return Ok(());
    }

    let response = client::send_request(Request::Shutdown)?;

    match response {
        Response::ShuttingDown => {
            println!("Daemon stopping...");

            // Wait for socket to disappear
            let sock = socket_path();
            for _ in 0..30 {
                thread::sleep(Duration::from_millis(100));
                if !sock.exists() {
                    println!("Daemon stopped");
                    return Ok(());
                }
            }

            // Force remove socket if daemon didn't clean up
            if sock.exists() {
                std::fs::remove_file(&sock).ok();
            }
            println!("Daemon stopped");
        }
        Response::Error(err) => {
            eprintln!("Error stopping daemon: {}", err.message);
        }
        _ => {
            eprintln!("Unexpected response");
        }
    }

    Ok(())
}
