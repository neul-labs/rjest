use serde::{Deserialize, Serialize};

/// Get the socket path for daemon communication
///
/// Uses UID only (not PID) so both CLI and daemon use the same path.
/// The daemon cleans up any stale socket on startup.
pub fn socket_path() -> std::path::PathBuf {
    let uid = unsafe { libc::getuid() };

    if let Ok(runtime_dir) = std::env::var("XDG_RUNTIME_DIR") {
        std::path::PathBuf::from(runtime_dir).join(format!("rjest-{}.sock", uid))
    } else {
        std::path::PathBuf::from(format!("/tmp/rjest-{}.sock", uid))
    }
}

/// Request sent from CLI to daemon
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Request {
    /// Run tests matching the given patterns
    Run(RunRequest),
    /// Start a watch session
    WatchStart(WatchStartRequest),
    /// Poll for changes in watch mode
    WatchPoll(WatchPollRequest),
    /// Stop a watch session
    WatchStop(WatchStopRequest),
    /// Ping the daemon to check if it's alive
    Ping,
    /// Get daemon status and cache statistics
    Status,
    /// Health check with detailed diagnostics
    Health,
    /// Shutdown the daemon
    Shutdown,
}

/// Response sent from daemon to CLI
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Response {
    /// Test run results
    Run(RunResponse),
    /// Watch session started with initial run results
    WatchStarted(WatchStartedResponse),
    /// Watch poll results
    WatchPoll(WatchPollResponse),
    /// Watch session stopped
    WatchStopped,
    /// Pong response to ping
    Pong,
    /// Daemon status information
    Status(StatusResponse),
    /// Health check response
    Health(HealthResponse),
    /// Acknowledgment of shutdown request
    ShuttingDown,
    /// Error occurred processing request
    Error(ErrorResponse),
}

/// Request to run tests
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunRequest {
    /// Absolute path to project root
    pub project_root: String,
    /// Test file patterns to match
    pub patterns: Vec<String>,
    /// CLI flags
    pub flags: RunFlags,
}

/// CLI flags for test runs
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RunFlags {
    /// Run tests serially in a single worker
    pub run_in_band: bool,
    /// Watch mode - re-run on file changes
    pub watch: bool,
    /// Exit after first test failure
    pub bail: bool,
    /// Output results as JSON
    pub json_output: bool,
    /// Machine-readable output for AI agents
    pub machine_output: bool,
    /// Maximum number of worker processes
    pub max_workers: Option<u32>,
    /// Path to Jest config file
    pub config_path: Option<String>,
    /// Only run tests affected by changed files
    pub only_changed: bool,
    /// Run tests related to specific source files
    pub find_related_tests: Vec<String>,
    /// Update snapshots
    pub update_snapshots: bool,
    /// Collect coverage
    pub coverage: bool,
    /// Filter by test name pattern
    pub test_name_pattern: Option<String>,
    /// Verbose output
    pub verbose: bool,
}

/// Request to start watch mode
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WatchStartRequest {
    /// Absolute path to project root
    pub project_root: String,
    /// Test file patterns to match
    pub patterns: Vec<String>,
    /// CLI flags (subset relevant to watch)
    pub flags: RunFlags,
}

/// Request to poll for changes in watch mode
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WatchPollRequest {
    /// Session ID from WatchStarted response
    pub session_id: String,
    /// Timeout in milliseconds for blocking wait
    pub timeout_ms: u64,
}

/// Request to stop watch mode
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WatchStopRequest {
    /// Session ID from WatchStarted response
    pub session_id: String,
}

/// Response when watch mode starts
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WatchStartedResponse {
    /// Session ID for subsequent poll/stop requests
    pub session_id: String,
    /// Initial test run results
    pub initial_run: RunResponse,
}

/// Response from watch poll
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WatchPollResponse {
    /// Whether files have changed since last poll
    pub has_changes: bool,
    /// New test run results (if has_changes is true)
    pub run_result: Option<RunResponse>,
    /// Files that changed
    pub changed_files: Vec<String>,
}

/// Response containing test results
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunResponse {
    /// Overall success (all tests passed)
    pub success: bool,
    /// Number of test suites that passed
    pub num_passed_suites: u32,
    /// Number of test suites that failed
    pub num_failed_suites: u32,
    /// Number of individual tests that passed
    pub num_passed_tests: u32,
    /// Number of individual tests that failed
    pub num_failed_tests: u32,
    /// Number of tests skipped
    pub num_skipped_tests: u32,
    /// Number of todo tests
    pub num_todo_tests: u32,
    /// Total execution time in milliseconds
    pub duration_ms: u64,
    /// Per-file results
    pub test_results: Vec<TestFileResult>,
    /// Snapshot summary
    pub snapshot_summary: Option<SnapshotSummary>,
}

/// Results for a single test file
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestFileResult {
    /// Absolute path to test file
    pub path: String,
    /// Whether this file passed
    pub passed: bool,
    /// Execution time for this file
    pub duration_ms: u64,
    /// Individual test results
    pub tests: Vec<TestResult>,
    /// Console output from this file
    pub console_output: Option<String>,
}

/// Result for a single test case
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestResult {
    /// Full test name including describe blocks
    pub name: String,
    /// Test status
    pub status: TestStatus,
    /// Execution time in milliseconds
    pub duration_ms: u64,
    /// Error details if failed
    pub error: Option<TestError>,
}

/// Test execution status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TestStatus {
    Passed,
    Failed,
    Skipped,
    Todo,
}

/// Error information for failed tests
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestError {
    /// Error message
    pub message: String,
    /// Stack trace
    pub stack: Option<String>,
    /// Diff for assertion failures
    pub diff: Option<String>,
    /// Source location
    pub location: Option<SourceLocation>,
}

/// Source code location
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceLocation {
    pub file: String,
    pub line: u32,
    pub column: Option<u32>,
}

/// Snapshot operation summary
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotSummary {
    pub added: u32,
    pub updated: u32,
    pub removed: u32,
    pub matched: u32,
    pub unmatched: u32,
    pub unchecked: u32,
}

/// Daemon status information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatusResponse {
    /// Daemon version
    pub version: String,
    /// Uptime in seconds
    pub uptime_secs: u64,
    /// Number of projects currently tracked
    pub projects_count: u32,
    /// Cache statistics
    pub cache_stats: CacheStats,
    /// Worker pool statistics
    pub worker_stats: WorkerStats,
}

/// Cache statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheStats {
    /// Number of cached transforms
    pub transform_count: u64,
    /// Size of transform cache in bytes
    pub transform_size_bytes: u64,
    /// Number of cached dependency graphs
    pub graph_count: u32,
    /// Cache hit rate (0.0 - 1.0)
    pub hit_rate: f64,
}

/// Worker pool statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkerStats {
    /// Number of active workers
    pub active: u32,
    /// Number of idle workers
    pub idle: u32,
    /// Total tests executed since daemon start
    pub total_tests_run: u64,
}

/// Health check response with detailed diagnostics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthResponse {
    /// Whether the daemon is healthy
    pub healthy: bool,
    /// Daemon version
    pub version: String,
    /// Uptime in seconds
    pub uptime_secs: u64,
    /// Response latency in microseconds (time to process health check)
    pub latency_us: u64,
    /// Memory usage in bytes (approximate)
    pub memory_bytes: u64,
    /// Detailed worker health information
    pub workers: Vec<WorkerHealth>,
    /// Number of active watch sessions
    pub watch_sessions: u32,
    /// Number of cached projects
    pub cached_projects: u32,
    /// Issues detected (empty if healthy)
    pub issues: Vec<String>,
}

/// Health information for a single worker
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkerHealth {
    /// Worker index
    pub id: u32,
    /// Whether the worker is alive
    pub alive: bool,
    /// Whether the worker is currently busy
    pub busy: bool,
    /// Number of tests run by this worker
    pub tests_run: u64,
    /// Time since last activity in seconds
    pub idle_secs: u64,
}

/// Error response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorResponse {
    /// Error code
    pub code: ErrorCode,
    /// Human-readable message
    pub message: String,
    /// Additional details
    pub details: Option<String>,
}

/// Error codes
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    /// Project configuration could not be loaded
    ConfigError,
    /// No test files found
    NoTestsFound,
    /// Transform/compilation failed
    TransformError,
    /// Worker crashed or timed out
    WorkerError,
    /// Internal daemon error
    InternalError,
    /// Invalid request
    InvalidRequest,
}

/// IPC address for nng
pub fn ipc_address() -> String {
    format!("ipc://{}", socket_path().display())
}
