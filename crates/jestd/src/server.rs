use anyhow::{Context, Result};
use nng::{Protocol, Socket};
use rjest_protocol::{
    ipc_address, socket_path, ErrorCode, ErrorResponse, Request, Response, RunResponse,
    StatusResponse, TestFileResult as ProtoTestFileResult, TestResult as ProtoTestResult,
    TestStatus, TestError as ProtoTestError,
    CacheStats as ProtoCacheStats, WorkerStats as ProtoWorkerStats, RunRequest,
    WatchStartRequest, WatchPollRequest, WatchStopRequest,
    WatchStartedResponse, WatchPollResponse, RunFlags,
    HealthResponse, WorkerHealth as ProtoWorkerHealth,
};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tracing::{debug, error, info, instrument, warn, span, Level};
use uuid::Uuid;

use crate::config::JestConfig;
use crate::discovery::TestDiscovery;
use crate::transform::Transformer;
use crate::watch::FileWatcher;
use crate::worker::{find_worker_script, WorkerConfig, WorkerPool};

/// Active watch session
struct WatchSession {
    project_root: PathBuf,
    patterns: Vec<String>,
    flags: RunFlags,
    watcher: FileWatcher,
    all_test_files: Vec<PathBuf>,
}

/// Daemon state shared across requests
struct DaemonState {
    start_time: Instant,
    running: AtomicBool,
    total_tests_run: AtomicU64,
    /// Cached configs per project root
    configs: Mutex<HashMap<PathBuf, JestConfig>>,
    /// Transform cache directory
    cache_dir: PathBuf,
    /// Active watch sessions
    watch_sessions: Mutex<HashMap<String, WatchSession>>,
    /// Persistent worker pool (keyed by project root for multi-project support)
    worker_pools: Mutex<HashMap<PathBuf, WorkerPool>>,
    /// Worker script path (cached)
    worker_script: Mutex<Option<PathBuf>>,
}

impl DaemonState {
    fn new() -> Self {
        let cache_dir = dirs::cache_dir()
            .unwrap_or_else(|| PathBuf::from("/tmp"))
            .join("rjest");

        Self {
            start_time: Instant::now(),
            running: AtomicBool::new(true),
            total_tests_run: AtomicU64::new(0),
            configs: Mutex::new(HashMap::new()),
            cache_dir,
            watch_sessions: Mutex::new(HashMap::new()),
            worker_pools: Mutex::new(HashMap::new()),
            worker_script: Mutex::new(None),
        }
    }

    /// Ensure the shared worker pool exists
    fn ensure_pool(&self, max_workers: usize) -> Result<()> {
        let mut pools = self.worker_pools.lock().unwrap();

        // Use a single shared pool (keyed by empty path)
        let key = PathBuf::new();
        if !pools.contains_key(&key) {
            // Get or cache worker script path
            let worker_script = {
                let mut script = self.worker_script.lock().unwrap();
                if script.is_none() {
                    *script = Some(find_worker_script()?);
                }
                script.clone().unwrap()
            };

            let pool = WorkerPool::new(max_workers, worker_script)?;
            pools.insert(key, pool);
        }

        Ok(())
    }

    /// Pre-spawn workers on daemon start for fast first request
    fn prewarm_workers(&self) {
        info!("Pre-warming worker pool...");
        if let Err(e) = self.ensure_pool(4) {
            warn!("Failed to pre-warm workers: {}", e);
        }
    }

    /// Cleanup idle workers to reduce memory usage
    fn cleanup_idle_workers(&self) {
        let mut pools = self.worker_pools.lock().unwrap();
        for pool in pools.values_mut() {
            pool.cleanup_idle_workers();
        }
    }

    /// Run tests using the shared worker pool
    fn run_tests_with_pool(
        &self,
        transforms: &[crate::transform::TransformResult],
        config: &WorkerConfig,
    ) -> Vec<Result<crate::worker::TestFileResult>> {
        let mut pools = self.worker_pools.lock().unwrap();
        let key = PathBuf::new();

        if let Some(pool) = pools.get_mut(&key) {
            pool.run_tests(transforms, config)
        } else {
            vec![Err(anyhow::anyhow!("No worker pool available"))]
        }
    }

    fn get_or_load_config(&self, project_root: &Path) -> Result<JestConfig> {
        let mut configs = self.configs.lock().unwrap();

        if let Some(config) = configs.get(project_root) {
            return Ok(config.clone());
        }

        let config = JestConfig::load(project_root)?;
        configs.insert(project_root.to_path_buf(), config.clone());
        Ok(config)
    }
}

/// Run the daemon RPC server
pub async fn run() -> Result<()> {
    let state = Arc::new(DaemonState::new());

    // Clean up any stale socket
    let sock_path = socket_path();
    if sock_path.exists() {
        std::fs::remove_file(&sock_path).ok();
    }

    // Create reply socket
    let socket = Socket::new(Protocol::Rep0).context("Failed to create nng socket")?;

    let addr = ipc_address();
    socket.listen(&addr).context("Failed to bind socket")?;
    info!("Listening on {}", addr);

    // Pre-warm worker pool in background (non-blocking)
    {
        let state_clone = Arc::clone(&state);
        std::thread::spawn(move || {
            state_clone.prewarm_workers();
        });
    }

    // Background thread to cleanup idle workers every 30 seconds
    {
        let state_clone = Arc::clone(&state);
        std::thread::spawn(move || {
            while state_clone.running.load(Ordering::Relaxed) {
                std::thread::sleep(std::time::Duration::from_secs(30));
                state_clone.cleanup_idle_workers();
            }
        });
    }

    // Handle requests
    while state.running.load(Ordering::Relaxed) {
        match socket.recv() {
            Ok(msg) => {
                let response = handle_request(&msg, &state);
                let response_bytes = serde_json::to_vec(&response).unwrap_or_else(|e| {
                    serde_json::to_vec(&Response::Error(ErrorResponse {
                        code: ErrorCode::InternalError,
                        message: format!("Failed to serialize response: {}", e),
                        details: None,
                    }))
                    .unwrap()
                });

                if let Err((_, e)) = socket.send(&response_bytes) {
                    error!("Failed to send response: {}", e);
                }
            }
            Err(e) => {
                if state.running.load(Ordering::Relaxed) {
                    error!("Failed to receive message: {}", e);
                }
            }
        }
    }

    info!("Daemon shutting down");
    Ok(())
}

fn handle_request(msg: &[u8], state: &Arc<DaemonState>) -> Response {
    let request: Request = match serde_json::from_slice(msg) {
        Ok(req) => req,
        Err(e) => {
            warn!("Invalid request: {}", e);
            return Response::Error(ErrorResponse {
                code: ErrorCode::InvalidRequest,
                message: format!("Failed to parse request: {}", e),
                details: None,
            });
        }
    };

    debug!("Received request: {:?}", request);

    match request {
        Request::Ping => {
            debug!("Handling ping");
            Response::Pong
        }

        Request::Status => {
            debug!("Handling status request");
            let configs = state.configs.lock().unwrap();
            Response::Status(StatusResponse {
                version: env!("CARGO_PKG_VERSION").to_string(),
                uptime_secs: state.start_time.elapsed().as_secs(),
                projects_count: configs.len() as u32,
                cache_stats: ProtoCacheStats {
                    transform_count: 0, // TODO: Get from transformer
                    transform_size_bytes: 0,
                    graph_count: configs.len() as u32,
                    hit_rate: 0.0,
                },
                worker_stats: ProtoWorkerStats {
                    active: 0,
                    idle: 0,
                    total_tests_run: state.total_tests_run.load(Ordering::Relaxed),
                },
            })
        }

        Request::Shutdown => {
            info!("Shutdown requested");
            state.running.store(false, Ordering::Relaxed);
            Response::ShuttingDown
        }

        Request::Health => {
            debug!("Handling health check");
            let health_start = Instant::now();

            let configs = state.configs.lock().unwrap();
            let watch_sessions = state.watch_sessions.lock().unwrap();

            // Check for issues
            let mut issues = Vec::new();
            let uptime = state.start_time.elapsed().as_secs();

            // Get memory usage (approximate via /proc/self/statm on Linux)
            let memory_bytes = get_memory_usage().unwrap_or(0);

            let latency_us = health_start.elapsed().as_micros() as u64;

            Response::Health(HealthResponse {
                healthy: issues.is_empty(),
                version: env!("CARGO_PKG_VERSION").to_string(),
                uptime_secs: uptime,
                latency_us,
                memory_bytes,
                workers: vec![], // Workers are created per-request currently
                watch_sessions: watch_sessions.len() as u32,
                cached_projects: configs.len() as u32,
                issues,
            })
        }

        Request::Run(run_request) => {
            match execute_tests(&run_request, state) {
                Ok(response) => Response::Run(response),
                Err(e) => {
                    error!("Test execution failed: {}", e);
                    Response::Error(ErrorResponse {
                        code: ErrorCode::InternalError,
                        message: e.to_string(),
                        details: Some(format!("{:?}", e)),
                    })
                }
            }
        }

        Request::WatchStart(watch_request) => {
            match start_watch_session(&watch_request, state) {
                Ok(response) => Response::WatchStarted(response),
                Err(e) => {
                    error!("Watch start failed: {}", e);
                    Response::Error(ErrorResponse {
                        code: ErrorCode::InternalError,
                        message: e.to_string(),
                        details: Some(format!("{:?}", e)),
                    })
                }
            }
        }

        Request::WatchPoll(poll_request) => {
            match poll_watch_session(&poll_request, state) {
                Ok(response) => Response::WatchPoll(response),
                Err(e) => {
                    error!("Watch poll failed: {}", e);
                    Response::Error(ErrorResponse {
                        code: ErrorCode::InternalError,
                        message: e.to_string(),
                        details: Some(format!("{:?}", e)),
                    })
                }
            }
        }

        Request::WatchStop(stop_request) => {
            stop_watch_session(&stop_request, state);
            Response::WatchStopped
        }
    }
}

#[instrument(skip(state), fields(project = %request.project_root))]
fn execute_tests(request: &RunRequest, state: &Arc<DaemonState>) -> Result<RunResponse> {
    let start_time = Instant::now();
    // Canonicalize project root to ensure consistent cache key lookups
    let project_root = PathBuf::from(&request.project_root)
        .canonicalize()
        .unwrap_or_else(|_| PathBuf::from(&request.project_root));

    // Record request in metrics
    crate::metrics::record_request();

    info!("Executing tests for {}", project_root.display());

    // Load configuration
    let config = state.get_or_load_config(&project_root)?;

    // Check for multi-project configuration
    if let Some(ref projects) = config.projects {
        if !projects.is_empty() {
            return execute_multi_project_tests(request, state, &project_root, projects);
        }
    }

    // Discover test files
    let discovery = TestDiscovery::new(config.clone());
    let test_files = if !request.flags.find_related_tests.is_empty() {
        let related: Vec<PathBuf> = request.flags.find_related_tests
            .iter()
            .map(PathBuf::from)
            .collect();
        discovery.find_related_tests(&related)?
    } else if request.flags.only_changed {
        // Get changed files from git and find related tests
        let changed_files = crate::git::get_changed_files(&project_root)?;
        if changed_files.is_empty() {
            info!("No changed files detected");
            vec![]
        } else {
            let all_tests = discovery.find_tests_matching(&request.patterns)?;
            crate::git::find_related_test_files(&changed_files, &all_tests)
        }
    } else {
        discovery.find_tests_matching(&request.patterns)?
    };

    if test_files.is_empty() {
        return Ok(RunResponse {
            success: true,
            num_passed_suites: 0,
            num_failed_suites: 0,
            num_passed_tests: 0,
            num_failed_tests: 0,
            num_skipped_tests: 0,
            num_todo_tests: 0,
            duration_ms: start_time.elapsed().as_millis() as u64,
            test_results: vec![],
            snapshot_summary: None,
        });
    }

    info!("Found {} test files", test_files.len());

    // Create transformer
    let transformer = Transformer::new(&state.cache_dir)?;

    // Transform test files in parallel
    let transform_results = transformer.transform_many(&test_files);
    let transforms: Vec<_> = transform_results
        .into_iter()
        .zip(test_files.iter())
        .filter_map(|(result, path)| {
            match result {
                Ok(t) => Some(t),
                Err(e) => {
                    warn!("Failed to transform {}: {}", path.display(), e);
                    None
                }
            }
        })
        .collect();

    // Create worker config
    let worker_config = WorkerConfig {
        root_dir: config.root_dir.clone(),
        setup_files: config.setup_files.clone(),
        setup_files_after_env: config.setup_files_after_env.clone(),
        test_timeout: config.test_timeout,
        clear_mocks: config.clear_mocks,
        reset_mocks: config.reset_mocks,
        restore_mocks: config.restore_mocks,
        update_snapshots: request.flags.update_snapshots,
        test_name_pattern: request.flags.test_name_pattern.clone(),
    };

    // Ensure worker pool exists (pre-warmed on daemon start)
    let max_workers = if request.flags.run_in_band {
        1
    } else {
        request.flags.max_workers.map(|w| w as usize).unwrap_or_else(|| config.max_workers_count())
    };
    state.ensure_pool(max_workers)?;

    // Run tests using shared pool
    let results = state.run_tests_with_pool(&transforms, &worker_config);

    // Aggregate results
    let mut test_results = Vec::new();
    let mut num_passed_suites = 0u32;
    let mut num_failed_suites = 0u32;
    let mut num_passed_tests = 0u32;
    let mut num_failed_tests = 0u32;
    let mut num_skipped_tests = 0u32;
    let mut num_todo_tests = 0u32;

    // Snapshot aggregation
    let mut snap_added = 0u32;
    let mut snap_updated = 0u32;
    let mut snap_matched = 0u32;
    let mut snap_unmatched = 0u32;

    for result in results {
        match result {
            Ok(file_result) => {
                if file_result.passed {
                    num_passed_suites += 1;
                } else {
                    num_failed_suites += 1;
                }

                // Aggregate snapshot stats
                if let Some(snap) = &file_result.snapshot_summary {
                    snap_added += snap.added;
                    snap_updated += snap.updated;
                    snap_matched += snap.matched;
                    snap_unmatched += snap.unmatched;
                }

                let tests: Vec<ProtoTestResult> = file_result
                    .tests
                    .into_iter()
                    .map(|t| {
                        let status = match t.status.as_str() {
                            "passed" => {
                                num_passed_tests += 1;
                                TestStatus::Passed
                            }
                            "failed" => {
                                num_failed_tests += 1;
                                TestStatus::Failed
                            }
                            "skipped" => {
                                num_skipped_tests += 1;
                                TestStatus::Skipped
                            }
                            "todo" => {
                                num_todo_tests += 1;
                                TestStatus::Todo
                            }
                            _ => TestStatus::Failed,
                        };

                        ProtoTestResult {
                            name: t.name,
                            status,
                            duration_ms: t.duration_ms,
                            error: t.error.map(|e| ProtoTestError {
                                message: e.message,
                                stack: e.stack,
                                diff: e.diff,
                                location: None,
                            }),
                        }
                    })
                    .collect();

                test_results.push(ProtoTestFileResult {
                    path: file_result.path,
                    passed: file_result.passed,
                    duration_ms: file_result.duration_ms,
                    tests,
                    console_output: None,
                });
            }
            Err(e) => {
                num_failed_suites += 1;
                warn!("Test file failed: {}", e);
            }
        }
    }

    // Update stats
    let total_tests = num_passed_tests + num_failed_tests + num_skipped_tests + num_todo_tests;
    state.total_tests_run.fetch_add(total_tests as u64, Ordering::Relaxed);

    let success = num_failed_tests == 0 && num_failed_suites == 0;
    let duration_ms = start_time.elapsed().as_millis() as u64;

    info!(
        "Tests complete: {} passed, {} failed in {}ms",
        num_passed_tests, num_failed_tests, duration_ms
    );

    // Record metrics
    crate::metrics::record_test_results(num_passed_tests as u64, num_failed_tests as u64);
    crate::metrics::record_test_file(duration_ms * 1000); // Convert to microseconds

    // Build snapshot summary if any snapshots were processed
    let snapshot_summary = if snap_added > 0 || snap_updated > 0 || snap_matched > 0 || snap_unmatched > 0 {
        Some(rjest_protocol::SnapshotSummary {
            added: snap_added,
            updated: snap_updated,
            removed: 0, // TODO: Track removed snapshots
            matched: snap_matched,
            unmatched: snap_unmatched,
            unchecked: 0, // TODO: Track unchecked snapshots
        })
    } else {
        None
    };

    Ok(RunResponse {
        success,
        num_passed_suites,
        num_failed_suites,
        num_passed_tests,
        num_failed_tests,
        num_skipped_tests,
        num_todo_tests,
        duration_ms,
        test_results,
        snapshot_summary,
    })
}

/// Start a new watch session
fn start_watch_session(
    request: &WatchStartRequest,
    state: &Arc<DaemonState>,
) -> Result<WatchStartedResponse> {
    let project_root = PathBuf::from(&request.project_root);
    info!("Starting watch session for {}", project_root.display());

    // Load configuration
    let config = state.get_or_load_config(&project_root)?;

    // Discover all test files
    let discovery = TestDiscovery::new(config.clone());
    let all_test_files = discovery.find_tests_matching(&request.patterns)?;

    // Create file watcher
    let mut watcher = FileWatcher::new()?;

    // Watch all roots
    for root in &config.roots {
        if root.exists() {
            watcher.watch(root)?;
        }
    }
    // Also watch the project root
    watcher.watch(&project_root)?;

    // Run initial tests
    let run_request = RunRequest {
        project_root: request.project_root.clone(),
        patterns: request.patterns.clone(),
        flags: request.flags.clone(),
    };
    let initial_run = execute_tests(&run_request, state)?;

    // Generate session ID
    let session_id = Uuid::new_v4().to_string();

    // Store session
    let session = WatchSession {
        project_root,
        patterns: request.patterns.clone(),
        flags: request.flags.clone(),
        watcher,
        all_test_files,
    };

    let mut sessions = state.watch_sessions.lock().unwrap();
    sessions.insert(session_id.clone(), session);

    info!("Watch session {} started", session_id);

    Ok(WatchStartedResponse {
        session_id,
        initial_run,
    })
}

/// Poll for changes in a watch session
fn poll_watch_session(
    request: &WatchPollRequest,
    state: &Arc<DaemonState>,
) -> Result<WatchPollResponse> {
    let timeout = Duration::from_millis(request.timeout_ms);

    // Get the session
    let mut sessions = state.watch_sessions.lock().unwrap();
    let session = sessions
        .get_mut(&request.session_id)
        .ok_or_else(|| anyhow::anyhow!("Watch session not found: {}", request.session_id))?;

    // Wait for changes with timeout
    let changed_files = session.watcher.wait_for_changes(timeout);

    if changed_files.is_empty() {
        return Ok(WatchPollResponse {
            has_changes: false,
            run_result: None,
            changed_files: vec![],
        });
    }

    info!("Detected {} changed files", changed_files.len());

    // Find affected tests
    let affected_tests = crate::watch::get_affected_tests(&changed_files, &session.all_test_files);

    let changed_file_strings: Vec<String> = changed_files
        .iter()
        .map(|p| p.to_string_lossy().to_string())
        .collect();

    if affected_tests.is_empty() {
        return Ok(WatchPollResponse {
            has_changes: true,
            run_result: None,
            changed_files: changed_file_strings,
        });
    }

    info!("Running {} affected tests", affected_tests.len());

    // Update the session's all_test_files (in case new test files were added)
    let config = state.get_or_load_config(&session.project_root)?;
    let discovery = TestDiscovery::new(config);
    if let Ok(new_test_files) = discovery.find_tests_matching(&session.patterns) {
        session.all_test_files = new_test_files;
    }

    // Re-run affected tests
    let test_patterns: Vec<String> = affected_tests
        .iter()
        .map(|p| p.to_string_lossy().to_string())
        .collect();

    let run_request = RunRequest {
        project_root: session.project_root.to_string_lossy().to_string(),
        patterns: test_patterns,
        flags: session.flags.clone(),
    };

    // Release the lock before executing tests
    drop(sessions);

    let run_result = execute_tests(&run_request, state)?;

    Ok(WatchPollResponse {
        has_changes: true,
        run_result: Some(run_result),
        changed_files: changed_file_strings,
    })
}

/// Stop a watch session
fn stop_watch_session(request: &WatchStopRequest, state: &Arc<DaemonState>) {
    let mut sessions = state.watch_sessions.lock().unwrap();
    if sessions.remove(&request.session_id).is_some() {
        info!("Watch session {} stopped", request.session_id);
    } else {
        warn!("Watch session {} not found", request.session_id);
    }
}

/// Execute tests across multiple projects in a monorepo
fn execute_multi_project_tests(
    request: &RunRequest,
    state: &Arc<DaemonState>,
    root_dir: &Path,
    projects: &[serde_json::Value],
) -> Result<RunResponse> {
    let start_time = Instant::now();

    info!("Running tests across {} projects", projects.len());

    // Aggregate results across all projects
    let mut all_test_results = Vec::new();
    let mut total_passed_suites = 0u32;
    let mut total_failed_suites = 0u32;
    let mut total_passed_tests = 0u32;
    let mut total_failed_tests = 0u32;
    let mut total_skipped_tests = 0u32;
    let mut total_todo_tests = 0u32;
    let mut total_snap_added = 0u32;
    let mut total_snap_updated = 0u32;
    let mut total_snap_matched = 0u32;
    let mut total_snap_unmatched = 0u32;

    for project in projects {
        // Extract project path from the config value
        let project_path = match project {
            serde_json::Value::String(s) => {
                // Project is a path string
                let path = if s.starts_with('<') {
                    // Handle <rootDir>/path patterns
                    s.replace("<rootDir>", &root_dir.to_string_lossy())
                } else if std::path::Path::new(s).is_absolute() {
                    s.clone()
                } else {
                    root_dir.join(s).to_string_lossy().to_string()
                };
                PathBuf::from(path)
            }
            serde_json::Value::Object(obj) => {
                // Project is an inline config - use rootDir if specified
                if let Some(serde_json::Value::String(root)) = obj.get("rootDir") {
                    if std::path::Path::new(root).is_absolute() {
                        PathBuf::from(root)
                    } else {
                        root_dir.join(root)
                    }
                } else {
                    // No rootDir specified, skip this project
                    warn!("Project config missing rootDir, skipping");
                    continue;
                }
            }
            _ => {
                warn!("Invalid project config type, skipping");
                continue;
            }
        };

        if !project_path.exists() {
            warn!("Project path does not exist: {}", project_path.display());
            continue;
        }

        info!("Running tests for project: {}", project_path.display());

        // Create a request for this specific project
        let project_request = RunRequest {
            project_root: project_path.to_string_lossy().to_string(),
            patterns: request.patterns.clone(),
            flags: request.flags.clone(),
        };

        // Execute tests for this project
        match execute_single_project_tests(&project_request, state) {
            Ok(result) => {
                total_passed_suites += result.num_passed_suites;
                total_failed_suites += result.num_failed_suites;
                total_passed_tests += result.num_passed_tests;
                total_failed_tests += result.num_failed_tests;
                total_skipped_tests += result.num_skipped_tests;
                total_todo_tests += result.num_todo_tests;

                if let Some(snap) = &result.snapshot_summary {
                    total_snap_added += snap.added;
                    total_snap_updated += snap.updated;
                    total_snap_matched += snap.matched;
                    total_snap_unmatched += snap.unmatched;
                }

                all_test_results.extend(result.test_results);
            }
            Err(e) => {
                warn!("Failed to run tests for project {}: {}", project_path.display(), e);
                total_failed_suites += 1;
            }
        }
    }

    let duration_ms = start_time.elapsed().as_millis() as u64;
    let success = total_failed_tests == 0 && total_failed_suites == 0;

    // Build snapshot summary if any snapshots were processed
    let snapshot_summary = if total_snap_added > 0 || total_snap_updated > 0
        || total_snap_matched > 0 || total_snap_unmatched > 0 {
        Some(rjest_protocol::SnapshotSummary {
            added: total_snap_added,
            updated: total_snap_updated,
            removed: 0,
            matched: total_snap_matched,
            unmatched: total_snap_unmatched,
            unchecked: 0,
        })
    } else {
        None
    };

    info!(
        "Multi-project tests complete: {} passed, {} failed in {}ms",
        total_passed_tests, total_failed_tests, duration_ms
    );

    Ok(RunResponse {
        success,
        num_passed_suites: total_passed_suites,
        num_failed_suites: total_failed_suites,
        num_passed_tests: total_passed_tests,
        num_failed_tests: total_failed_tests,
        num_skipped_tests: total_skipped_tests,
        num_todo_tests: total_todo_tests,
        duration_ms,
        test_results: all_test_results,
        snapshot_summary,
    })
}

/// Execute tests for a single project (extracted from execute_tests)
fn execute_single_project_tests(request: &RunRequest, state: &Arc<DaemonState>) -> Result<RunResponse> {
    let start_time = Instant::now();
    // Canonicalize project root to ensure consistent cache key lookups
    let project_root = PathBuf::from(&request.project_root)
        .canonicalize()
        .unwrap_or_else(|_| PathBuf::from(&request.project_root));

    // Load configuration for this project
    let config = state.get_or_load_config(&project_root)?;

    // Discover test files
    let discovery = TestDiscovery::new(config.clone());
    let test_files = if !request.flags.find_related_tests.is_empty() {
        let related: Vec<PathBuf> = request.flags.find_related_tests
            .iter()
            .map(PathBuf::from)
            .collect();
        discovery.find_related_tests(&related)?
    } else if request.flags.only_changed {
        // Get changed files from git and find related tests
        let changed_files = crate::git::get_changed_files(&project_root)?;
        if changed_files.is_empty() {
            info!("No changed files detected");
            vec![]
        } else {
            let all_tests = discovery.find_tests_matching(&request.patterns)?;
            crate::git::find_related_test_files(&changed_files, &all_tests)
        }
    } else {
        discovery.find_tests_matching(&request.patterns)?
    };

    if test_files.is_empty() {
        return Ok(RunResponse {
            success: true,
            num_passed_suites: 0,
            num_failed_suites: 0,
            num_passed_tests: 0,
            num_failed_tests: 0,
            num_skipped_tests: 0,
            num_todo_tests: 0,
            duration_ms: start_time.elapsed().as_millis() as u64,
            test_results: vec![],
            snapshot_summary: None,
        });
    }

    // Create transformer
    let transformer = Transformer::new(&state.cache_dir)?;

    // Transform test files in parallel
    let transform_results = transformer.transform_many(&test_files);
    let transforms: Vec<_> = transform_results
        .into_iter()
        .zip(test_files.iter())
        .filter_map(|(result, path)| {
            match result {
                Ok(t) => Some(t),
                Err(e) => {
                    warn!("Failed to transform {}: {}", path.display(), e);
                    None
                }
            }
        })
        .collect();

    // Create worker config
    let worker_config = WorkerConfig {
        root_dir: config.root_dir.clone(),
        setup_files: config.setup_files.clone(),
        setup_files_after_env: config.setup_files_after_env.clone(),
        test_timeout: config.test_timeout,
        clear_mocks: config.clear_mocks,
        reset_mocks: config.reset_mocks,
        restore_mocks: config.restore_mocks,
        update_snapshots: request.flags.update_snapshots,
        test_name_pattern: request.flags.test_name_pattern.clone(),
    };

    // Ensure worker pool exists (pre-warmed on daemon start)
    let max_workers = if request.flags.run_in_band {
        1
    } else {
        request.flags.max_workers.map(|w| w as usize).unwrap_or_else(|| config.max_workers_count())
    };
    state.ensure_pool(max_workers)?;

    // Run tests using shared pool
    let results = state.run_tests_with_pool(&transforms, &worker_config);

    // Aggregate results
    let mut test_results = Vec::new();
    let mut num_passed_suites = 0u32;
    let mut num_failed_suites = 0u32;
    let mut num_passed_tests = 0u32;
    let mut num_failed_tests = 0u32;
    let mut num_skipped_tests = 0u32;
    let mut num_todo_tests = 0u32;
    let mut snap_added = 0u32;
    let mut snap_updated = 0u32;
    let mut snap_matched = 0u32;
    let mut snap_unmatched = 0u32;

    for result in results {
        match result {
            Ok(file_result) => {
                if file_result.passed {
                    num_passed_suites += 1;
                } else {
                    num_failed_suites += 1;
                }

                if let Some(snap) = &file_result.snapshot_summary {
                    snap_added += snap.added;
                    snap_updated += snap.updated;
                    snap_matched += snap.matched;
                    snap_unmatched += snap.unmatched;
                }

                let tests: Vec<ProtoTestResult> = file_result
                    .tests
                    .into_iter()
                    .map(|t| {
                        let status = match t.status.as_str() {
                            "passed" => {
                                num_passed_tests += 1;
                                TestStatus::Passed
                            }
                            "failed" => {
                                num_failed_tests += 1;
                                TestStatus::Failed
                            }
                            "skipped" => {
                                num_skipped_tests += 1;
                                TestStatus::Skipped
                            }
                            "todo" => {
                                num_todo_tests += 1;
                                TestStatus::Todo
                            }
                            _ => TestStatus::Failed,
                        };

                        ProtoTestResult {
                            name: t.name,
                            status,
                            duration_ms: t.duration_ms,
                            error: t.error.map(|e| ProtoTestError {
                                message: e.message,
                                stack: e.stack,
                                diff: e.diff,
                                location: None,
                            }),
                        }
                    })
                    .collect();

                test_results.push(ProtoTestFileResult {
                    path: file_result.path,
                    passed: file_result.passed,
                    duration_ms: file_result.duration_ms,
                    tests,
                    console_output: None,
                });
            }
            Err(e) => {
                num_failed_suites += 1;
                warn!("Test file failed: {}", e);
            }
        }
    }

    let total_tests = num_passed_tests + num_failed_tests + num_skipped_tests + num_todo_tests;
    state.total_tests_run.fetch_add(total_tests as u64, Ordering::Relaxed);

    let snapshot_summary = if snap_added > 0 || snap_updated > 0 || snap_matched > 0 || snap_unmatched > 0 {
        Some(rjest_protocol::SnapshotSummary {
            added: snap_added,
            updated: snap_updated,
            removed: 0,
            matched: snap_matched,
            unmatched: snap_unmatched,
            unchecked: 0,
        })
    } else {
        None
    };

    Ok(RunResponse {
        success: num_failed_tests == 0 && num_failed_suites == 0,
        num_passed_suites,
        num_failed_suites,
        num_passed_tests,
        num_failed_tests,
        num_skipped_tests,
        num_todo_tests,
        duration_ms: start_time.elapsed().as_millis() as u64,
        test_results,
        snapshot_summary,
    })
}

/// Get approximate memory usage of the current process
fn get_memory_usage() -> Option<u64> {
    // On Linux, read from /proc/self/statm
    #[cfg(target_os = "linux")]
    {
        if let Ok(content) = std::fs::read_to_string("/proc/self/statm") {
            let parts: Vec<&str> = content.split_whitespace().collect();
            if let Some(rss) = parts.get(1) {
                if let Ok(pages) = rss.parse::<u64>() {
                    // Page size is typically 4KB
                    return Some(pages * 4096);
                }
            }
        }
    }

    // On other platforms, return 0 for now
    #[cfg(not(target_os = "linux"))]
    {
        None
    }

    #[cfg(target_os = "linux")]
    None
}
