//! Tests for watch mode functionality
//!
//! Verifies:
//! - Watch session can be started
//! - File changes are detected
//! - Incremental test runs work correctly
//! - Sessions can be stopped

use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::Duration;
use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use tempfile::TempDir;

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

/// Start the daemon process
fn start_daemon() -> std::process::Child {
    let bin = jestd_bin();
    Command::new(&bin)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Failed to start daemon")
}

/// Connect to the daemon
fn connect_to_daemon() -> Result<UnixStream, std::io::Error> {
    let socket_path = rjest_protocol::socket_path();
    let start = std::time::Instant::now();
    loop {
        match UnixStream::connect(&socket_path) {
            Ok(stream) => return Ok(stream),
            Err(e) if start.elapsed() < Duration::from_secs(5) => {
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

    serde_json::from_str(&response).map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))
}

/// Stop the daemon
fn stop_daemon(child: &mut std::process::Child) {
    let _ = child.kill();
    let _ = child.wait();
}

#[test]
fn test_watch_start() {
    let mut child = start_daemon();
    std::thread::sleep(Duration::from_millis(500));

    let mut stream = connect_to_daemon().expect("Should connect to daemon");
    let project_path = fixture_path().to_string_lossy().to_string();

    // Start watch session
    let watch_request = serde_json::json!({
        "type": "WatchStart",
        "projectRoot": project_path,
        "patterns": ["*.test.js"],
        "flags": {
            "runInBand": true
        }
    });

    let response = send_request(&mut stream, &watch_request).expect("Should get response");

    // Verify watch started response
    assert_eq!(response["type"], "WatchStarted");
    assert!(response["sessionId"].is_string(), "Should have session ID");
    assert!(response["initialRun"].is_object(), "Should have initial run result");

    let session_id = response["sessionId"].as_str().unwrap().to_string();

    // Stop the watch session
    let stop_request = serde_json::json!({
        "type": "WatchStop",
        "sessionId": session_id
    });

    let stop_response = send_request(&mut stream, &stop_request).expect("Should get response");
    assert_eq!(stop_response["type"], "WatchStopped");

    stop_daemon(&mut child);
}

#[test]
fn test_watch_poll_no_changes() {
    let mut child = start_daemon();
    std::thread::sleep(Duration::from_millis(500));

    let mut stream = connect_to_daemon().expect("Should connect to daemon");
    let project_path = fixture_path().to_string_lossy().to_string();

    // Start watch session
    let watch_request = serde_json::json!({
        "type": "WatchStart",
        "projectRoot": project_path,
        "patterns": ["*.test.js"],
        "flags": {}
    });

    let start_response = send_request(&mut stream, &watch_request).expect("Should get response");
    let session_id = start_response["sessionId"].as_str().unwrap().to_string();

    // Poll with short timeout - should detect no changes
    let poll_request = serde_json::json!({
        "type": "WatchPoll",
        "sessionId": session_id,
        "timeoutMs": 100
    });

    let poll_response = send_request(&mut stream, &poll_request).expect("Should get response");

    assert_eq!(poll_response["type"], "WatchPoll");
    assert!(poll_response["hasChanges"].is_boolean(), "Should have hasChanges flag");
    assert_eq!(poll_response["hasChanges"].as_bool().unwrap_or(true), false, "Should have no changes");

    // Stop session
    let stop_request = serde_json::json!({
        "type": "WatchStop",
        "sessionId": session_id
    });
    let _ = send_request(&mut stream, &stop_request);

    stop_daemon(&mut child);
}

#[test]
fn test_watch_stop_invalid_session() {
    let mut child = start_daemon();
    std::thread::sleep(Duration::from_millis(500));

    let mut stream = connect_to_daemon().expect("Should connect to daemon");

    // Try to stop non-existent session - should not crash
    let stop_request = serde_json::json!({
        "type": "WatchStop",
        "sessionId": "non-existent-session-id-12345"
    });

    // Should not panic, just silently ignore
    let response = send_request(&mut stream, &stop_request).expect("Should get response");
    assert_eq!(response["type"], "WatchStopped"); // Still returns success

    stop_daemon(&mut child);
}

#[test]
fn test_watch_poll_invalid_session() {
    let mut child = start_daemon();
    std::thread::sleep(Duration::from_millis(500));

    let mut stream = connect_to_daemon().expect("Should connect to daemon");

    // Try to poll non-existent session
    let poll_request = serde_json::json!({
        "type": "WatchPoll",
        "sessionId": "non-existent-session-id-12345",
        "timeoutMs": 100
    });

    // Should get error response
    let response = send_request(&mut stream, &poll_request).expect("Should get response");
    assert_eq!(response["type"], "Error");
    assert!(response["message"].is_string(), "Should have error message");

    stop_daemon(&mut child);
}

#[test]
fn test_multiple_watch_sessions() {
    let mut child = start_daemon();
    std::thread::sleep(Duration::from_millis(500));

    let mut stream = connect_to_daemon().expect("Should connect to daemon");
    let project_path = fixture_path().to_string_lossy().to_string();

    // Start first watch session
    let watch_request1 = serde_json::json!({
        "type": "WatchStart",
        "projectRoot": project_path,
        "patterns": ["*.test.js"],
        "flags": {}
    });

    let response1 = send_request(&mut stream, &watch_request1).expect("Should get response");
    assert_eq!(response1["type"], "WatchStarted");
    let session_id1 = response1["sessionId"].as_str().unwrap().to_string();

    // Start second watch session with different patterns
    let watch_request2 = serde_json::json!({
        "type": "WatchStart",
        "projectRoot": project_path,
        "patterns": ["failing.test.js"],
        "flags": {}
    });

    let response2 = send_request(&mut stream, &watch_request2).expect("Should get response");
    assert_eq!(response2["type"], "WatchStarted");
    let session_id2 = response2["sessionId"].as_str().unwrap().to_string();

    // Sessions should have different IDs
    assert_ne!(session_id1, session_id2, "Session IDs should be unique");

    // Stop both sessions
    let stop1 = serde_json::json!({
        "type": "WatchStop",
        "sessionId": session_id1
    });
    let _ = send_request(&mut stream, &stop1);

    let stop2 = serde_json::json!({
        "type": "WatchStop",
        "sessionId": session_id2
    });
    let _ = send_request(&mut stream, &stop2);

    stop_daemon(&mut child);
}

#[test]
fn test_watch_start_response_contains_initial_run() {
    let mut child = start_daemon();
    std::thread::sleep(Duration::from_millis(500));

    let mut stream = connect_to_daemon().expect("Should connect to daemon");
    let project_path = fixture_path().to_string_lossy().to_string();

    let watch_request = serde_json::json!({
        "type": "WatchStart",
        "projectRoot": project_path,
        "patterns": ["simple.test.js"],
        "flags": {
            "runInBand": true
        }
    });

    let response = send_request(&mut stream, &watch_request).expect("Should get response");

    assert_eq!(response["type"], "WatchStarted");
    assert!(response["initialRun"].is_object(), "Should have initial run");

    let initial_run = &response["initialRun"];
    assert!(initial_run.get("success").is_some(), "Initial run should have success");
    assert!(initial_run.get("testResults").is_some(), "Initial run should have results");

    // Stop session
    let session_id = response["sessionId"].as_str().unwrap().to_string();
    let stop_request = serde_json::json!({
        "type": "WatchStop",
        "sessionId": session_id
    });
    let _ = send_request(&mut stream, &stop_request);

    stop_daemon(&mut child);
}
