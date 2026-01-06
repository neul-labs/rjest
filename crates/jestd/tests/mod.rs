//! Integration tests for CLI to daemon workflow
//!
//! These tests verify the full pipeline: CLI requests → daemon → workers → Jest runtime

pub mod daemon_startup;
pub mod run_request;
pub mod watch_session;
