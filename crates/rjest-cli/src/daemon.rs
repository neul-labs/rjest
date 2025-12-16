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

/// Health check - detailed diagnostics
pub fn health() -> Result<()> {
    if !client::ping()? {
        println!("Health: \x1b[31mUNHEALTHY\x1b[0m");
        println!("Status: Daemon is not running");
        std::process::exit(1);
    }

    let response = client::send_request(Request::Health)?;

    match response {
        Response::Health(health) => {
            // Health status
            if health.healthy {
                println!("Health: \x1b[32mHEALTHY\x1b[0m");
            } else {
                println!("Health: \x1b[31mUNHEALTHY\x1b[0m");
            }

            println!();
            println!("Version:         {}", health.version);
            println!("Uptime:          {}s", health.uptime_secs);
            println!("Response time:   {} us", health.latency_us);
            println!("Memory:          {} KB", health.memory_bytes / 1024);
            println!();
            println!("Resources:");
            println!("  Cached projects:  {}", health.cached_projects);
            println!("  Watch sessions:   {}", health.watch_sessions);

            if !health.workers.is_empty() {
                println!();
                println!("Workers:");
                for worker in &health.workers {
                    let status = if !worker.alive {
                        "\x1b[31mDEAD\x1b[0m"
                    } else if worker.busy {
                        "\x1b[33mBUSY\x1b[0m"
                    } else {
                        "\x1b[32mIDLE\x1b[0m"
                    };
                    println!(
                        "  Worker {}: {} - {} tests run, idle {}s",
                        worker.id, status, worker.tests_run, worker.idle_secs
                    );
                }
            }

            if !health.issues.is_empty() {
                println!();
                println!("\x1b[31mIssues:\x1b[0m");
                for issue in &health.issues {
                    println!("  - {}", issue);
                }
            }

            if !health.healthy {
                std::process::exit(1);
            }
        }
        Response::Error(err) => {
            eprintln!("Error getting health: {}", err.message);
            std::process::exit(1);
        }
        _ => {
            eprintln!("Unexpected response");
            std::process::exit(1);
        }
    }

    Ok(())
}
