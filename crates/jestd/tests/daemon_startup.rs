//! Tests for daemon startup and basic communication
//!
//! Verifies:
//! - Daemon starts and creates socket file
//! - Ping/pong communication works
//! - Shutdown works correctly
//! - Health check returns valid response

use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// Fixture project path for tests
fn fixture_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("minimal-project")
}

/// Check if the jestd binary exists
fn jestd_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("target")
        .join("debug")
        .join("jestd")
}

/// Start the daemon process and return its handle along with the socket path
fn start_daemon() -> (std::process::Child, PathBuf, String) {
    let bin = jestd_bin();

    // Ensure binary exists, build if needed
    if !bin.exists() {
        panic!("jestd binary not found at {:?}. Run `cargo build` first.", bin);
    }

    let mut child = Command::new(&bin)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Failed to start daemon");

    // Read stdout to get the socket path from the listening message
    let mut stdout = child.stdout.take().unwrap();
    let mut buffer = String::new();
    let _ = stdout.read_to_string(&mut buffer);

    // Extract socket path from "Listening on ipc://..." message
    let socket_path = if let Some(start) = buffer.find("ipc://") {
        let addr = &buffer[start + 6..];
        if let Some(end) = addr.find('\n') {
            addr[..end].trim().to_string()
        } else {
            addr.trim().to_string()
        }
    } else {
        // Fallback to default socket path
        rjest_protocol::socket_path().to_string_lossy().to_string()
    };

    // Give daemon a bit more time to be ready
    std::thread::sleep(Duration::from_millis(200));

    (child, bin, socket_path)
}

/// Connect to the daemon via IPC socket using the provided socket path
fn connect_to_daemon(socket_path: &str) -> Result<UnixStream, std::io::Error> {
    // Retry connection a few times
    let start = Instant::now();
    loop {
        match UnixStream::connect(socket_path) {
            Ok(stream) => return Ok(stream),
            Err(_) if start.elapsed() < Duration::from_secs(5) => {
                std::thread::sleep(Duration::from_millis(100));
            }
            Err(e) => return Err(e),
        }
    }
}

/// Send a JSON request and get response
fn send_request(stream: &mut UnixStream, request: &serde_json::Value) -> Result<serde_json::Value, std::io::Error> {
    let request_str = request.to_string() + "\n";
    stream.write_all(request_str.as_bytes())?;

    let mut response = String::new();
    stream.read_to_string(&mut response)?;

    // Parse JSON response
    serde_json::from_str(&response).map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))
}

/// Stop the daemon gracefully
fn stop_daemon(child: &mut std::process::Child) {
    let _ = child.kill();
    let _ = child.wait();
}

#[test]
fn test_daemon_starts_and_creates_socket() {
    let (mut child, _bin, socket_path) = start_daemon();

    // Check socket file exists
    let socket_path_buf = PathBuf::from(&socket_path);
    assert!(socket_path_buf.exists(), "Socket file should exist at {:?}", socket_path);

    stop_daemon(&mut child);

    // Socket should be cleaned up on drop
}

#[test]
fn test_ping_pong() {
    let (mut child, _bin, socket_path) = start_daemon();
    let mut stream = connect_to_daemon(&socket_path).expect("Should connect to daemon");

    // Send ping request
    let ping_request = serde_json::json!({
        "type": "Ping"
    });

    let response = send_request(&mut stream, &ping_request).expect("Should get response");

    // Verify pong response
    assert_eq!(response["type"], "Pong");

    stop_daemon(&mut child);
}

#[test]
fn test_shutdown() {
    let (mut child, _bin, socket_path) = start_daemon();
    let mut stream = connect_to_daemon(&socket_path).expect("Should connect to daemon");

    // Send shutdown request
    let shutdown_request = serde_json::json!({
        "type": "Shutdown"
    });

    let response = send_request(&mut stream, &shutdown_request).expect("Should get response");

    // Verify shutting down response
    assert_eq!(response["type"], "ShuttingDown");

    // Daemon should exit
    let status = child.wait().expect("Should wait for daemon");
    assert!(status.success(), "Daemon should exit successfully");

    // Socket should be cleaned up
    let socket_path_buf = PathBuf::from(&socket_path);
    assert!(!socket_path_buf.exists(), "Socket should be cleaned up after shutdown");
}

#[test]
fn test_status_request() {
    let (mut child, _bin, socket_path) = start_daemon();
    let mut stream = connect_to_daemon(&socket_path).expect("Should connect to daemon");

    // Send status request
    let status_request = serde_json::json!({
        "type": "Status"
    });

    let response = send_request(&mut stream, &status_request).expect("Should get response");

    // Verify status response structure
    assert_eq!(response["type"], "Status");
    assert!(response["version"].is_string(), "Should have version");
    assert!(response["uptime_secs"].is_number(), "Should have uptime");
    assert!(response["projects_count"].is_number(), "Should have projects count");
    assert!(response["worker_stats"].is_object(), "Should have worker stats");

    stop_daemon(&mut child);
}

#[test]
fn test_health_request() {
    let (mut child, _bin, socket_path) = start_daemon();
    let mut stream = connect_to_daemon(&socket_path).expect("Should connect to daemon");

    // Send health request
    let health_request = serde_json::json!({
        "type": "Health"
    });

    let response = send_request(&mut stream, &health_request).expect("Should get response");

    // Verify health response structure
    assert_eq!(response["type"], "Health");
    assert!(response["healthy"].is_boolean(), "Should have healthy flag");
    assert!(response["version"].is_string(), "Should have version");
    assert!(response["uptime_secs"].is_number(), "Should have uptime");
    assert!(response["workers"].is_array(), "Should have workers array");

    stop_daemon(&mut child);
}

#[test]
fn test_invalid_request_returns_error() {
    let (mut child, _bin, socket_path) = start_daemon();
    let mut stream = connect_to_daemon(&socket_path).expect("Should connect to daemon");

    // Send invalid request (malformed JSON)
    let invalid_request = serde_json::json!({
        "type": "NonExistentRequest"
    });

    let response = send_request(&mut stream, &invalid_request).expect("Should get response");

    // Should get error response for unknown request type
    assert_eq!(response["type"], "Error");
    assert!(response["message"].is_string(), "Should have error message");

    stop_daemon(&mut child);
}

#[test]
fn test_multiple_requests_same_connection() {
    let (mut child, _bin, socket_path) = start_daemon();
    let mut stream = connect_to_daemon(&socket_path).expect("Should connect to daemon");

    // Send multiple ping requests on same connection
    for _ in 0..3 {
        let ping_request = serde_json::json!({
            "type": "Ping"
        });
        let response = send_request(&mut stream, &ping_request).expect("Should get response");
        assert_eq!(response["type"], "Pong");
    }

    stop_daemon(&mut child);
}

#[test]
fn test_run_with_empty_patterns() {
    let (mut child, _bin, socket_path) = start_daemon();
    let mut stream = connect_to_daemon(&socket_path).expect("Should connect to daemon");

    let project_path = fixture_path().to_string_lossy().to_string();

    // Send run request with empty patterns (should use config defaults)
    let run_request = serde_json::json!({
        "type": "Run",
        "projectRoot": project_path,
        "patterns": [],
        "flags": {
            "runInBand": true
        }
    });

    let response = send_request(&mut stream, &run_request).expect("Should get response");

    // Should get run response
    assert_eq!(response["type"], "Run");

    stop_daemon(&mut child);
}
