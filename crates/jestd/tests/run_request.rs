//! Tests for test execution via daemon
//!
//! Verifies:
//! - Running tests produces correct results
//! - Pattern matching works correctly
//! - Test results are properly formatted
//! - Error handling for invalid projects

use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::Duration;
use std::io::{Read, Write};
use std::os::unix::net::UnixStream;

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
fn test_run_simple_test_file() {
    let mut child = start_daemon();
    std::thread::sleep(Duration::from_millis(500)); // Wait for daemon startup

    let mut stream = connect_to_daemon().expect("Should connect to daemon");
    let project_path = fixture_path().to_string_lossy().to_string();

    // Run specific test file
    let run_request = serde_json::json!({
        "type": "Run",
        "projectRoot": project_path,
        "patterns": ["simple.test.js"],
        "flags": {
            "runInBand": true
        }
    });

    let response = send_request(&mut stream, &run_request).expect("Should get response");

    // Verify response structure
    assert_eq!(response["type"], "Run");
    assert!(response["success"].is_boolean(), "Should have success flag");

    // Check test results
    if response["success"].as_bool().unwrap_or(false) {
        // If tests passed, verify test counts
        assert!(response["numPassedTests"].as_u64().unwrap_or(0) > 0, "Should have passed tests");
        assert_eq!(response["numFailedTests"].as_u64().unwrap_or(0), 0, "Should have no failed tests");
    }

    stop_daemon(&mut child);
}

#[test]
fn test_run_with_pattern_matching() {
    let mut child = start_daemon();
    std::thread::sleep(Duration::from_millis(500));

    let mut stream = connect_to_daemon().expect("Should connect to daemon");
    let project_path = fixture_path().to_string_lossy().to_string();

    // Run tests matching pattern
    let run_request = serde_json::json!({
        "type": "Run",
        "projectRoot": project_path,
        "patterns": ["**/*.test.js"],
        "flags": {
            "runInBand": true
        }
    });

    let response = send_request(&mut stream, &run_request).expect("Should get response");

    assert_eq!(response["type"], "Run");
    // Should find at least the simple.test.js file
    let test_count = response["numPassedTests"].as_u64().unwrap_or(0)
        + response["numFailedTests"].as_u64().unwrap_or(0)
        + response["numSkippedTests"].as_u64().unwrap_or(0);
    assert!(test_count >= 5, "Should find multiple tests in test files");

    stop_daemon(&mut child);
}

#[test]
fn test_run_non_existent_project() {
    let mut child = start_daemon();
    std::thread::sleep(Duration::from_millis(500));

    let mut stream = connect_to_daemon().expect("Should connect to daemon");

    // Try to run tests in non-existent project
    let run_request = serde_json::json!({
        "type": "Run",
        "projectRoot": "/tmp/this-project-does-not-exist-12345",
        "patterns": ["*.test.js"],
        "flags": {
            "runInBand": true
        }
    });

    let response = send_request(&mut stream, &run_request).expect("Should get response");

    // Should get error response
    assert_eq!(response["type"], "Error");
    assert!(response["message"].is_string(), "Should have error message");

    stop_daemon(&mut child);
}

#[test]
fn test_run_with_test_name_pattern() {
    let mut child = start_daemon();
    std::thread::sleep(Duration::from_millis(500));

    let mut stream = connect_to_daemon().expect("Should connect to daemon");
    let project_path = fixture_path().to_string_lossy().to_string();

    // Run only tests matching name pattern
    let run_request = serde_json::json!({
        "type": "Run",
        "projectRoot": project_path,
        "patterns": ["*.test.js"],
        "flags": {
            "runInBand": true,
            "testNamePattern": "should pass"
        }
    });

    let response = send_request(&mut stream, &run_request).expect("Should get response");

    assert_eq!(response["type"], "Run");

    stop_daemon(&mut child);
}

#[test]
fn test_run_in_band_vs_parallel() {
    let project_path = fixture_path().to_string_lossy().to_string();
    let run_request_base = serde_json::json!({
        "type": "Run",
        "projectRoot": project_path,
        "patterns": ["simple.test.js"],
        "flags": {
            "runInBand": true
        }
    });

    // Run in band (sequential)
    let mut child = start_daemon();
    std::thread::sleep(Duration::from_millis(500));
    let mut stream = connect_to_daemon().expect("Should connect");

    let response_in_band = send_request(&mut stream, &run_request_base).expect("Should get response");
    assert_eq!(response_in_band["type"], "Run");

    stop_daemon(&mut child);

    // Run with runInBand = false (parallel - will use default workers)
    let run_request_parallel = serde_json::json!({
        "type": "Run",
        "projectRoot": project_path,
        "patterns": ["simple.test.js"],
        "flags": {
            "runInBand": false
        }
    });

    let mut child2 = start_daemon();
    std::thread::sleep(Duration::from_millis(500));
    let mut stream2 = connect_to_daemon().expect("Should connect");

    let response_parallel = send_request(&mut stream2, &run_request_parallel).expect("Should get response");
    assert_eq!(response_parallel["type"], "Run");

    stop_daemon(&mut child2);
}

#[test]
fn test_run_response_has_correct_structure() {
    let mut child = start_daemon();
    std::thread::sleep(Duration::from_millis(500));

    let mut stream = connect_to_daemon().expect("Should connect to daemon");
    let project_path = fixture_path().to_string_lossy().to_string();

    let run_request = serde_json::json!({
        "type": "Run",
        "projectRoot": project_path,
        "patterns": ["simple.test.js"],
        "flags": {
            "runInBand": true
        }
    });

    let response = send_request(&mut stream, &run_request).expect("Should get response");

    // Verify all expected fields are present
    assert!(response.get("type").is_some(), "Should have type field");
    assert!(response.get("success").is_some(), "Should have success field");
    assert!(response.get("numPassedSuites").is_some(), "Should have passed suites count");
    assert!(response.get("numFailedSuites").is_some(), "Should have failed suites count");
    assert!(response.get("numPassedTests").is_some(), "Should have passed tests count");
    assert!(response.get("numFailedTests").is_some(), "Should have failed tests count");
    assert!(response.get("durationMs").is_some(), "Should have duration");
    assert!(response.get("testResults").is_some(), "Should have test results array");

    stop_daemon(&mut child);
}

#[test]
fn test_run_with_no_matching_tests() {
    let mut child = start_daemon();
    std::thread::sleep(Duration::from_millis(500));

    let mut stream = connect_to_daemon().expect("Should connect to daemon");
    let project_path = fixture_path().to_string_lossy().to_string();

    // Request non-matching pattern
    let run_request = serde_json::json!({
        "type": "Run",
        "projectRoot": project_path,
        "patterns": ["**/nonexistent-*.test.js"],
        "flags": {
            "runInBand": true
        }
    });

    let response = send_request(&mut stream, &run_request).expect("Should get response");

    assert_eq!(response["type"], "Run");
    assert!(response["success"].as_bool().unwrap_or(false), "Should be success with no tests");
    assert_eq!(response["numPassedTests"].as_u64().unwrap_or(0), 0, "Should have 0 tests");
    assert_eq!(response["numFailedTests"].as_u64().unwrap_or(0), 0, "Should have 0 failed tests");

    stop_daemon(&mut child);
}
