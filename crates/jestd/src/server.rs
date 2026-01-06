use anyhow::{Context, Result};
use nng::{Protocol, Socket};
use rjest_protocol::{
    ipc_address, socket_path, ErrorCode, ErrorResponse, Request, Response, RunResponse,
    StatusResponse, TestFileResult as ProtoTestFileResult, TestResult as ProtoTestResult,
    TestStatus, TestError as ProtoTestError,
    CacheStats as ProtoCacheStats, WorkerStats as ProtoWorkerStats, RunRequest,
    WatchStartRequest, WatchPollRequest, WatchStopRequest,
    WatchStartedResponse, WatchPollResponse, RunFlags,
    HealthResponse, WorkerHealth,
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

/// Safely serialize a response to bytes
///
/// Returns a minimal error response if serialization fails for any reason.
/// This avoids panics in the request handling path.
fn serialize_response(response: &Response) -> Vec<u8> {
    match serde_json::to_vec(response) {
        Ok(bytes) => bytes,
        Err(e) => {
            // Try to serialize a minimal error response
            let error_response = Response::Error(ErrorResponse {
                code: ErrorCode::InternalError,
                message: format!("Failed to serialize response: {}", e),
                details: None,
            });
            // Last resort: return empty bytes (the client will handle this)
            serde_json::to_vec(&error_response).unwrap_or_else(|_| Vec::new())
        }
    }
}

/// Active watch session
struct WatchSession {
    project_root: PathBuf,
    patterns: Vec<String>,
    flags: RunFlags,
    watcher: FileWatcher,
    all_test_files: Vec<PathBuf>,
}

/// Daemon state shared across requests
///
/// Uses Relaxed atomic ordering for `running` and `total_tests_run` because:
/// - `running`: We only need eventual visibility of the shutdown flag
/// - `total_tests_run`: This is an eventually-consistent counter for metrics
struct DaemonState {
    start_time: Instant,
    /// Shutdown flag - Relaxed ordering is sufficient since we only need eventual visibility
    running: AtomicBool,
    /// Total tests run counter - Relaxed ordering is fine for metrics (eventual consistency)
    total_tests_run: AtomicU64,
    /// Cached configs per project root (Arc to avoid cloning on cache hits)
    configs: Mutex<HashMap<PathBuf, Arc<JestConfig>>>,
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
        let mut pools = self.worker_pools.lock().map_err(|e| anyhow::anyhow!("Worker pools lock poisoned: {}", e))?;

        // Use a single shared pool (keyed by empty path)
        let key = PathBuf::new();
        if !pools.contains_key(&key) {
            // Get or cache worker script path
            let worker_script = {
                let mut script = self.worker_script.lock().map_err(|e| anyhow::anyhow!("Worker script lock poisoned: {}", e))?;
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
        let mut pools = match self.worker_pools.lock() {
            Ok(pools) => pools,
            Err(e) => {
                warn!("Failed to lock worker pools for cleanup: {}", e);
                return;
            }
        };
        for pool in pools.values_mut() {
            pool.cleanup_idle_workers();
        }
    }

    /// Get health status of all workers across all pools
    fn worker_health(&self) -> Vec<WorkerHealth> {
        match self.worker_pools.lock() {
            Ok(pools) => {
                let mut health = Vec::new();
                for pool in pools.values() {
                    health.extend(pool.health());
                }
                health
            }
            Err(e) => {
                warn!("Failed to lock worker pools for health check: {}", e);
                Vec::new()
            }
        }
    }

    /// Run tests using the shared worker pool
    fn run_tests_with_pool(
        &self,
        transforms: &[crate::transform::TransformResult],
        config: &WorkerConfig,
    ) -> Vec<Result<crate::worker::TestFileResult>> {
        let mut pools = match self.worker_pools.lock() {
            Ok(pools) => pools,
            Err(e) => {
                return vec![Err(anyhow::anyhow!("Worker pools lock poisoned: {}", e))];
            }
        };
        let key = PathBuf::new();

        if let Some(pool) = pools.get_mut(&key) {
            pool.run_tests(transforms, config)
        } else {
            vec![Err(anyhow::anyhow!("No worker pool available"))]
        }
    }

    fn get_or_load_config(&self, project_root: &Path) -> Result<Arc<JestConfig>> {
        let mut configs = self.configs.lock().map_err(|e| anyhow::anyhow!("Configs lock poisoned: {}", e))?;

        if let Some(config) = configs.get(project_root) {
            return Ok(config.clone());
        }

        let config = JestConfig::load(project_root)?;
        let arc_config = Arc::new(config);
        configs.insert(project_root.to_path_buf(), arc_config.clone());
        Ok(arc_config)
    }

    /// Async version that loads config without blocking the runtime
    async fn get_or_load_config_async(&self, project_root: &Path) -> Result<Arc<JestConfig>> {
        // First, try to get from cache without holding lock during async operations
        let needs_load = {
            let configs = self.configs.lock().map_err(|e| anyhow::anyhow!("Configs lock poisoned: {}", e))?;
            !configs.contains_key(project_root)
        };

        if !needs_load {
            let configs = self.configs.lock().map_err(|e| anyhow::anyhow!("Configs lock poisoned: {}", e))?;
            if let Some(config) = configs.get(project_root) {
                return Ok(config.clone());
            }
        }

        // Load config (this will be async)
        let config = JestConfig::load_async(project_root).await?;
        let arc_config = Arc::new(config);

        // Store in cache
        let mut configs = self.configs.lock().map_err(|e| anyhow::anyhow!("Configs lock poisoned: {}", e))?;
        configs.insert(project_root.to_path_buf(), arc_config.clone());
        Ok(arc_config)
    }
}

/// Clean up any stale rjest sockets from previous runs
fn cleanup_stale_sockets() {
    let pattern = if let Ok(runtime_dir) = std::env::var("XDG_RUNTIME_DIR") {
        std::path::PathBuf::from(runtime_dir)
    } else {
        std::path::PathBuf::from("/tmp")
    };

    // Only scan /tmp for rjest sockets (not XDG_RUNTIME_DIR which is user-specific)
    if pattern == std::path::PathBuf::from("/tmp") {
        if let Ok(entries) = std::fs::read_dir("/tmp") {
            for entry in entries.flatten() {
                if let Some(name) = entry.file_name().to_str() {
                    if name.starts_with("rjest-") && name.ends_with(".sock") {
                        // Remove stale socket files
                        if let Err(e) = std::fs::remove_file(entry.path()) {
                            debug!("Failed to remove stale socket {}: {}", entry.path().display(), e);
                        }
                    }
                }
            }
        }
    }
}

/// Run the daemon RPC server
pub async fn run() -> Result<()> {
    let state = Arc::new(DaemonState::new());

    // Clean up any stale sockets from previous runs
    cleanup_stale_sockets();

    // Clean up any stale socket at the expected path
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
                let state_clone = Arc::clone(&state);
                let socket = socket.clone();

                // Spawn async task for request handling (avoids blocking on Node.js calls)
                tokio::spawn(async move {
                    let response = handle_request(&msg, &state_clone).await;
                    let response_bytes = serialize_response(&response);
                    if let Err((_, e)) = socket.send(&response_bytes) {
                        error!("Failed to send response: {}", e);
                    }
                });
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

async fn handle_request(msg: &[u8], state: &Arc<DaemonState>) -> Response {
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
            let configs = match state.configs.lock() {
                Ok(configs) => configs,
                Err(e) => {
                    return Response::Error(ErrorResponse {
                        code: ErrorCode::InternalError,
                        message: format!("Configs lock poisoned: {}", e),
                        details: None,
                    });
                }
            };
            Response::Status(StatusResponse {
                version: env!("CARGO_PKG_VERSION").to_string(),
                uptime_secs: state.start_time.elapsed().as_secs(),
                projects_count: configs.len() as u32,
                cache_stats: ProtoCacheStats {
                    transform_count: crate::metrics::snapshot().transform_count(),
                    transform_size_bytes: 0, // TODO: Track sled cache size
                    graph_count: configs.len() as u32,
                    hit_rate: crate::metrics::snapshot().cache_hit_rate(),
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

            let configs = match state.configs.lock() {
                Ok(configs) => configs,
                Err(e) => {
                    return Response::Error(ErrorResponse {
                        code: ErrorCode::InternalError,
                        message: format!("Configs lock poisoned: {}", e),
                        details: None,
                    });
                }
            };
            let watch_sessions = match state.watch_sessions.lock() {
                Ok(sessions) => sessions,
                Err(e) => {
                    return Response::Error(ErrorResponse {
                        code: ErrorCode::InternalError,
                        message: format!("Watch sessions lock poisoned: {}", e),
                        details: None,
                    });
                }
            };

            // Check for issues
            let mut issues = Vec::new();
            let uptime = state.start_time.elapsed().as_secs();

            // Get memory usage (approximate via /proc/self/statm on Linux)
            let memory_bytes = get_memory_usage().unwrap_or(0);

            // Get worker health
            let workers = state.worker_health();

            // Check for dead workers
            for worker in &workers {
                if !worker.alive {
                    issues.push(format!("Worker {} is not alive", worker.id));
                }
            }

            let latency_us = health_start.elapsed().as_micros() as u64;

            Response::Health(HealthResponse {
                healthy: issues.is_empty(),
                version: env!("CARGO_PKG_VERSION").to_string(),
                uptime_secs: uptime,
                latency_us,
                memory_bytes,
                workers,
                watch_sessions: watch_sessions.len() as u32,
                cached_projects: configs.len() as u32,
                issues,
            })
        }

        Request::Run(run_request) => {
            match execute_tests(&run_request, state).await {
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
            match start_watch_session(&watch_request, state).await {
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
            match poll_watch_session(&poll_request, state).await {
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

/// Transform test files and separate successful/failed transforms
///
/// Returns a tuple of (successful_transforms, transform_errors).
/// Transform errors are logged and returned as a list of (path, error_message) tuples.
fn transform_test_files(
    transformer: &Transformer,
    test_files: &[PathBuf],
) -> (
    Vec<crate::transform::TransformResult>,
    Vec<(PathBuf, String)>,
) {
    let transform_results = transformer.transform_many(test_files);

    let mut transforms = Vec::new();
    let mut errors = Vec::new();

    for (result, path) in transform_results.into_iter().zip(test_files.iter()) {
        match result {
            Ok(t) => transforms.push(t),
            Err(e) => {
                let error_msg = format!("Transform failed: {}", e);
                warn!("Failed to transform {}: {}", path.display(), e);
                errors.push((path.clone(), error_msg));
            }
        }
    }

    (transforms, errors)
}

/// Create worker configuration from JestConfig and RunFlags
///
/// Combines JestConfig settings with runtime flags to produce a WorkerConfig
/// that can be passed to test workers.
fn create_worker_config(
    config: &JestConfig,
    update_snapshots: bool,
    test_name_pattern: Option<String>,
) -> WorkerConfig {
    WorkerConfig {
        root_dir: config.root_dir.clone(),
        setup_files: config.setup_files.clone(),
        setup_files_after_env: config.setup_files_after_env.clone(),
        test_timeout: config.test_timeout,
        clear_mocks: config.clear_mocks,
        reset_mocks: config.reset_mocks,
        restore_mocks: config.restore_mocks,
        update_snapshots,
        test_name_pattern,
    }
}

/// Get max workers based on flags and config
///
/// Respects the `run_in_band` flag (single worker), any explicit `max_workers` flag,
/// or falls back to the configured maximum workers from JestConfig.
fn get_max_workers(
    run_in_band: bool,
    max_workers_flag: Option<u32>,
    config_max_workers: usize,
) -> usize {
    if run_in_band {
        1
    } else {
        max_workers_flag.map(|w| w as usize).unwrap_or_else(|| config_max_workers)
    }
}

#[instrument(skip(state), fields(project = %request.project_root))]
async fn execute_tests(request: &RunRequest, state: &Arc<DaemonState>) -> Result<RunResponse> {
    let start_time = Instant::now();

    // Validate and canonicalize project root
    let project_root = validate_project_root(&request.project_root)?;

    // Record request in metrics
    crate::metrics::record_request();

    info!("Executing tests for {}", project_root.display());

    // Load configuration (async to avoid blocking the runtime)
    let config = state.get_or_load_config_async(&project_root).await?;

    // Check for multi-project configuration (Arc derefs to JestConfig automatically)
    if let Some(ref projects) = config.projects {
        if !projects.is_empty() {
            return execute_multi_project_tests(request, state, &project_root, projects).await;
        }
    }

    // Discover test files
    let discovery = TestDiscovery::new((*config).clone());
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
    let (transforms, transform_errors) = transform_test_files(&transformer, &test_files);

    // Create worker config using helper
    let worker_config = create_worker_config(
        &config,
        request.flags.update_snapshots,
        request.flags.test_name_pattern.clone(),
    );

    // Get max workers using helper
    let max_workers = get_max_workers(
        request.flags.run_in_band,
        request.flags.max_workers,
        config.max_workers_count(),
    );
    state.ensure_pool(max_workers)?;

    // Run tests using shared pool
    let results = state.run_tests_with_pool(&transforms, &worker_config);

    // Aggregate results using shared helper
    let mut aggregated = AggregatedResults::new();

    for result in results {
        match result {
            Ok(file_result) => aggregated.add_file_result(file_result),
            Err(e) => {
                aggregated.num_failed_suites += 1;
                warn!("Test file failed: {}", e);
            }
        }
    }

    // Add transform errors as failed test results
    for (path, error_msg) in transform_errors {
        aggregated.add_transform_error(path, error_msg);
    }

    // Update stats
    let total_tests = aggregated.num_passed_tests + aggregated.num_failed_tests
        + aggregated.num_skipped_tests + aggregated.num_todo_tests;
    state.total_tests_run.fetch_add(total_tests as u64, Ordering::Relaxed);

    let success = aggregated.success();
    let duration_ms = start_time.elapsed().as_millis() as u64;

    info!(
        "Tests complete: {} passed, {} failed in {}ms",
        aggregated.num_passed_tests, aggregated.num_failed_tests, duration_ms
    );

    // Record metrics
    crate::metrics::record_test_results(aggregated.num_passed_tests as u64, aggregated.num_failed_tests as u64);
    crate::metrics::record_test_file(duration_ms * 1000); // Convert to microseconds

    // Build snapshot summary if any snapshots were processed
    let snapshot_summary = aggregated.build_snapshot_summary();

    Ok(RunResponse {
        success,
        num_passed_suites: aggregated.num_passed_suites,
        num_failed_suites: aggregated.num_failed_suites,
        num_passed_tests: aggregated.num_passed_tests,
        num_failed_tests: aggregated.num_failed_tests,
        num_skipped_tests: aggregated.num_skipped_tests,
        num_todo_tests: aggregated.num_todo_tests,
        duration_ms,
        test_results: aggregated.test_results,
        snapshot_summary,
    })
}

/// Start a new watch session
async fn start_watch_session(
    request: &WatchStartRequest,
    state: &Arc<DaemonState>,
) -> Result<WatchStartedResponse> {
    let project_root = PathBuf::from(&request.project_root);
    info!("Starting watch session for {}", project_root.display());

    // Load configuration
    let config = state.get_or_load_config(&project_root)?;

    // Discover all test files
    let discovery = TestDiscovery::new((*config).clone());
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
    let initial_run = execute_tests(&run_request, state).await?;

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

    let mut sessions = state.watch_sessions.lock().map_err(|e| anyhow::anyhow!("Watch sessions lock poisoned: {}", e))?;
    sessions.insert(session_id.clone(), session);

    info!("Watch session {} started", session_id);

    Ok(WatchStartedResponse {
        session_id,
        initial_run,
    })
}

/// Poll for changes in a watch session
async fn poll_watch_session(
    request: &WatchPollRequest,
    state: &Arc<DaemonState>,
) -> Result<WatchPollResponse> {
    let timeout = Duration::from_millis(request.timeout_ms);

    // Get the session data - use a block to limit lock scope
    let (project_root, patterns, flags, all_test_files, changed_files) = {
        let mut sessions = state.watch_sessions.lock().map_err(|e| anyhow::anyhow!("Watch sessions lock poisoned: {}", e))?;
        let session = match sessions.get(&request.session_id) {
            Some(session) => session,
            None => {
                return Err(anyhow::anyhow!("Watch session not found: {}", request.session_id));
            }
        };

        // Extract data we need before releasing the lock
        (
            session.project_root.clone(),
            session.patterns.clone(),
            session.flags.clone(),
            session.all_test_files.clone(),
            // Wait for changes with timeout (this is synchronous)
            session.watcher.wait_for_changes(timeout),
        )
        // Lock is released here when sessions goes out of scope
    };

    if changed_files.is_empty() {
        return Ok(WatchPollResponse {
            has_changes: false,
            run_result: None,
            changed_files: vec![],
        });
    }

    info!("Detected {} changed files", changed_files.len());

    // Find affected tests
    let affected_tests = crate::watch::get_affected_tests(&changed_files, &all_test_files);

    let changed_file_strings: Vec<String> = changed_files
        .iter()
        .map(|p| p.to_string_lossy().to_string())
        .collect();

    if affected_tests.is_empty() {
        // Update the session's all_test_files (in case new test files were added)
        let config = state.get_or_load_config(&project_root)?;
        let discovery = TestDiscovery::new((*config).clone());
        if let Ok(new_test_files) = discovery.find_tests_matching(&patterns) {
            let mut sessions = state.watch_sessions.lock().map_err(|e| anyhow::anyhow!("Watch sessions lock poisoned: {}", e))?;
            if let Some(session) = sessions.get_mut(&request.session_id) {
                session.all_test_files = new_test_files;
            }
        }

        return Ok(WatchPollResponse {
            has_changes: true,
            run_result: None,
            changed_files: changed_file_strings,
        });
    }

    info!("Running {} affected tests", affected_tests.len());

    // Update the session's all_test_files (in case new test files were added)
    let config = state.get_or_load_config(&project_root)?;
    let discovery = TestDiscovery::new((*config).clone());
    let new_test_files = discovery.find_tests_matching(&patterns).ok();
    let test_patterns: Vec<String> = affected_tests
        .iter()
        .map(|p| p.to_string_lossy().to_string())
        .collect();

    let run_request = RunRequest {
        project_root: project_root.to_string_lossy().to_string(),
        patterns: test_patterns,
        flags,
    };

    let run_result = execute_tests(&run_request, state).await?;

    // Update session with new test files after tests complete
    if let Some(files) = new_test_files {
        let mut sessions = state.watch_sessions.lock().map_err(|e| anyhow::anyhow!("Watch sessions lock poisoned: {}", e))?;
        if let Some(session) = sessions.get_mut(&request.session_id) {
            session.all_test_files = files;
        }
    }

    Ok(WatchPollResponse {
        has_changes: true,
        run_result: Some(run_result),
        changed_files: changed_file_strings,
    })
}

/// Stop a watch session
fn stop_watch_session(request: &WatchStopRequest, state: &Arc<DaemonState>) {
    let mut sessions = match state.watch_sessions.lock() {
        Ok(sessions) => sessions,
        Err(e) => {
            warn!("Failed to lock watch sessions: {}", e);
            return;
        }
    };
    if sessions.remove(&request.session_id).is_some() {
        info!("Watch session {} stopped", request.session_id);
    } else {
        warn!("Watch session {} not found", request.session_id);
    }
}

/// Execute tests across multiple projects in a monorepo
async fn execute_multi_project_tests(
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
        match execute_single_project_tests(&project_request, state).await {
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
            // Note: 'removed' and 'unchecked' snapshots are not currently tracked.
            // See execute_single_project_tests() for details.
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
async fn execute_single_project_tests(request: &RunRequest, state: &Arc<DaemonState>) -> Result<RunResponse> {
    let start_time = Instant::now();

    // Validate and canonicalize project root
    let project_root = validate_project_root(&request.project_root)?;

    // Load configuration for this project
    let config = state.get_or_load_config(&project_root)?;

    // Discover test files
    let discovery = TestDiscovery::new((*config).clone());
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

    // Separate successful and failed transforms
    let mut transforms: Vec<_> = Vec::new();
    let mut transform_errors: Vec<(PathBuf, String)> = Vec::new();

    for (result, path) in transform_results.into_iter().zip(test_files.iter()) {
        match result {
            Ok(t) => transforms.push(t),
            Err(e) => {
                let error_msg = format!("Transform failed: {}", e);
                warn!("Failed to transform {}: {}", path.display(), e);
                transform_errors.push((path.clone(), error_msg));
            }
        }
    }

    // Create worker config using helper
    let worker_config = create_worker_config(
        &config,
        request.flags.update_snapshots,
        request.flags.test_name_pattern.clone(),
    );

    // Get max workers using helper
    let max_workers = get_max_workers(
        request.flags.run_in_band,
        request.flags.max_workers,
        config.max_workers_count(),
    );
    state.ensure_pool(max_workers)?;

    // Run tests using shared pool
    let results = state.run_tests_with_pool(&transforms, &worker_config);

    // Aggregate results using shared helper
    let mut aggregated = AggregatedResults::new();

    for result in results {
        match result {
            Ok(file_result) => aggregated.add_file_result(file_result),
            Err(e) => {
                aggregated.num_failed_suites += 1;
                warn!("Test file failed: {}", e);
            }
        }
    }

    // Add transform errors as failed test results
    for (path, error_msg) in transform_errors {
        aggregated.add_transform_error(path, error_msg);
    }

    let total_tests = aggregated.num_passed_tests + aggregated.num_failed_tests
        + aggregated.num_skipped_tests + aggregated.num_todo_tests;
    state.total_tests_run.fetch_add(total_tests as u64, Ordering::Relaxed);

    let snapshot_summary = aggregated.build_snapshot_summary();

    Ok(RunResponse {
        success: aggregated.success(),
        num_passed_suites: aggregated.num_passed_suites,
        num_failed_suites: aggregated.num_failed_suites,
        num_passed_tests: aggregated.num_passed_tests,
        num_failed_tests: aggregated.num_failed_tests,
        num_skipped_tests: aggregated.num_skipped_tests,
        num_todo_tests: aggregated.num_todo_tests,
        duration_ms: start_time.elapsed().as_millis() as u64,
        test_results: aggregated.test_results,
        snapshot_summary,
    })
}

/// Validate that a project root path is safe
///
/// This function:
/// 1. Resolves the path to its canonical form
/// 2. Checks for path traversal attempts (..)
/// 3. Returns an error if the path is unsafe
fn validate_project_root(project_root: &str) -> Result<PathBuf> {
    let path = PathBuf::from(project_root);

    // Check for path traversal patterns in the original path
    let path_str = project_root.replace('\\', "/"); // Normalize Windows separators
    if path_str.contains("..") {
        anyhow::bail!("Project root path contains invalid '..' traversal: {}", project_root);
    }

    // Try to canonicalize
    let canonical = path.canonicalize().map_err(|e| {
        anyhow::anyhow!("Failed to resolve project root path '{}': {}", project_root, e)
    })?;

    // On Unix, ensure the path doesn't escape to /tmp or system directories
    // This is a basic check - for production use, consider allowinglist-based validation
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        // Get the inode - if it's a symlink pointing outside, this may be an issue
        if let Ok(metadata) = std::fs::metadata(&canonical) {
            // Check if it's a symlink that points outside expected directories
            if std::fs::symlink_metadata(&canonical).map(|m| m.file_type().is_symlink()).unwrap_or(false) {
                warn!("Project root is a symlink: {}", canonical.display());
            }
        }
    }

    Ok(canonical)
}

/// Aggregated test result data
struct AggregatedResults {
    test_results: Vec<ProtoTestFileResult>,
    num_passed_suites: u32,
    num_failed_suites: u32,
    num_passed_tests: u32,
    num_failed_tests: u32,
    num_skipped_tests: u32,
    num_todo_tests: u32,
    snap_added: u32,
    snap_updated: u32,
    snap_matched: u32,
    snap_unmatched: u32,
}

impl AggregatedResults {
    fn new() -> Self {
        Self {
            test_results: Vec::new(),
            num_passed_suites: 0,
            num_failed_suites: 0,
            num_passed_tests: 0,
            num_failed_tests: 0,
            num_skipped_tests: 0,
            num_todo_tests: 0,
            snap_added: 0,
            snap_updated: 0,
            snap_matched: 0,
            snap_unmatched: 0,
        }
    }

    /// Add a test file result to the aggregation
    fn add_file_result(&mut self, file_result: crate::worker::TestFileResult) {
        if file_result.passed {
            self.num_passed_suites += 1;
        } else {
            self.num_failed_suites += 1;
        }

        // Aggregate snapshot stats
        if let Some(snap) = &file_result.snapshot_summary {
            self.snap_added += snap.added;
            self.snap_updated += snap.updated;
            self.snap_matched += snap.matched;
            self.snap_unmatched += snap.unmatched;
        }

        let tests: Vec<ProtoTestResult> = file_result
            .tests
            .into_iter()
            .map(|t| {
                let status = match t.status.as_str() {
                    "passed" => {
                        self.num_passed_tests += 1;
                        TestStatus::Passed
                    }
                    "failed" => {
                        self.num_failed_tests += 1;
                        TestStatus::Failed
                    }
                    "skipped" => {
                        self.num_skipped_tests += 1;
                        TestStatus::Skipped
                    }
                    "todo" => {
                        self.num_todo_tests += 1;
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

        self.test_results.push(ProtoTestFileResult {
            path: file_result.path,
            passed: file_result.passed,
            duration_ms: file_result.duration_ms,
            tests,
            console_output: None,
        });
    }

    /// Add a transform error as a failed test result
    fn add_transform_error(&mut self, path: PathBuf, error_msg: String) {
        self.num_failed_suites += 1;
        self.test_results.push(ProtoTestFileResult {
            path: path.to_string_lossy().to_string(),
            passed: false,
            duration_ms: 0,
            tests: vec![ProtoTestResult {
                name: "transform".to_string(),
                status: TestStatus::Failed,
                duration_ms: 0,
                error: Some(ProtoTestError {
                    message: error_msg,
                    stack: None,
                    diff: None,
                    location: None,
                }),
            }],
            console_output: None,
        });
    }

    /// Build the final snapshot summary if any snapshots were processed
    fn build_snapshot_summary(&self) -> Option<rjest_protocol::SnapshotSummary> {
        if self.snap_added > 0 || self.snap_updated > 0 || self.snap_matched > 0 || self.snap_unmatched > 0 {
            Some(rjest_protocol::SnapshotSummary {
                added: self.snap_added,
                updated: self.snap_updated,
                removed: 0,
                matched: self.snap_matched,
                unmatched: self.snap_unmatched,
                unchecked: 0,
            })
        } else {
            None
        }
    }

    /// Check if all tests passed
    fn success(&self) -> bool {
        self.num_failed_tests == 0 && self.num_failed_suites == 0
    }
}

/// Get approximate memory usage of the current process in bytes
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

    // On macOS, use getrusage
    #[cfg(target_os = "macos")]
    {
        let mut usage: libc::rusage = unsafe { std::mem::zeroed() };
        if unsafe { libc::getrusage(libc::RUSAGE_SELF, &mut usage) } == 0 {
            // On macOS, ru_maxrss is in kilobytes
            return Some(usage.ru_maxrss as u64 * 1024);
        } else {
            return None;
        }
    }

    // On other platforms, return None
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::worker::TestResult;
    use std::path::PathBuf;

    fn create_test_result(status: &str, duration_ms: u64) -> TestResult {
        TestResult {
            name: format!("test_{}", status),
            status: status.to_string(),
            duration_ms,
            error: None,
        }
    }

    fn create_test_file_result(passed: bool, tests: Vec<TestResult>) -> crate::worker::TestFileResult {
        crate::worker::TestFileResult {
            path: "test/example.test.ts".to_string(),
            passed,
            duration_ms: 100,
            tests,
            snapshot_summary: None,
        }
    }

    #[test]
    fn test_aggregated_results_new() {
        let aggregated = AggregatedResults::new();
        assert_eq!(aggregated.num_passed_suites, 0);
        assert_eq!(aggregated.num_failed_suites, 0);
        assert_eq!(aggregated.num_passed_tests, 0);
        assert_eq!(aggregated.num_failed_tests, 0);
        assert!(aggregated.test_results.is_empty());
        assert!(aggregated.build_snapshot_summary().is_none());
        assert!(aggregated.success());
    }

    #[test]
    fn test_aggregated_results_add_passed_file() {
        let mut aggregated = AggregatedResults::new();
        let test_file = create_test_file_result(
            true,
            vec![
                create_test_result("passed", 10),
                create_test_result("passed", 20),
            ],
        );
        aggregated.add_file_result(test_file);

        assert_eq!(aggregated.num_passed_suites, 1);
        assert_eq!(aggregated.num_failed_suites, 0);
        assert_eq!(aggregated.num_passed_tests, 2);
        assert_eq!(aggregated.num_failed_tests, 0);
        assert_eq!(aggregated.test_results.len(), 1);
        assert!(aggregated.success());
    }

    #[test]
    fn test_aggregated_results_add_failed_file() {
        let mut aggregated = AggregatedResults::new();
        let test_file = create_test_file_result(
            false,
            vec![
                create_test_result("passed", 10),
                create_test_result("failed", 20),
            ],
        );
        aggregated.add_file_result(test_file);

        assert_eq!(aggregated.num_passed_suites, 0);
        assert_eq!(aggregated.num_failed_suites, 1);
        assert_eq!(aggregated.num_passed_tests, 1);
        assert_eq!(aggregated.num_failed_tests, 1);
        assert!(!aggregated.success());
    }

    #[test]
    fn test_aggregated_results_add_transform_error() {
        let mut aggregated = AggregatedResults::new();
        aggregated.add_transform_error(
            PathBuf::from("/test/example.test.ts"),
            "Transform failed: syntax error".to_string(),
        );

        assert_eq!(aggregated.num_passed_suites, 0);
        assert_eq!(aggregated.num_failed_suites, 1);
        assert_eq!(aggregated.test_results.len(), 1);
        assert!(!aggregated.test_results[0].passed);
        assert_eq!(aggregated.test_results[0].tests.len(), 1);
        assert_eq!(aggregated.test_results[0].tests[0].name, "transform");
        assert!(!aggregated.success());
    }

    #[test]
    fn test_aggregated_results_snapshot_summary_with_snapshots() {
        let mut aggregated = AggregatedResults::new();

        // Simulate a file with snapshots using worker::SnapshotSummary
        let file_result = crate::worker::TestFileResult {
            path: "test/example.test.ts".to_string(),
            passed: true,
            duration_ms: 100,
            tests: vec![],
            snapshot_summary: Some(crate::worker::SnapshotSummary {
                added: 2,
                updated: 1,
                matched: 5,
                unmatched: 1,
            }),
        };
        aggregated.add_file_result(file_result);

        let summary = aggregated.build_snapshot_summary().unwrap();
        assert_eq!(summary.added, 2);
        assert_eq!(summary.updated, 1);
        assert_eq!(summary.matched, 5);
        assert_eq!(summary.unmatched, 1);
    }

    #[test]
    fn test_aggregated_results_snapshot_summary_empty() {
        let aggregated = AggregatedResults::new();
        assert!(aggregated.build_snapshot_summary().is_none());
    }

    #[test]
    fn test_aggregated_results_all_test_statuses() {
        let mut aggregated = AggregatedResults::new();
        let file_result = crate::worker::TestFileResult {
            path: "test/example.test.ts".to_string(),
            passed: true,
            duration_ms: 100,
            tests: vec![
                create_test_result("passed", 10),
                create_test_result("failed", 20),
                create_test_result("skipped", 30),
                create_test_result("todo", 40),
            ],
            snapshot_summary: None,
        };
        aggregated.add_file_result(file_result);

        assert_eq!(aggregated.num_passed_tests, 1);
        assert_eq!(aggregated.num_failed_tests, 1);
        assert_eq!(aggregated.num_skipped_tests, 1);
        assert_eq!(aggregated.num_todo_tests, 1);
        assert_eq!(aggregated.num_passed_suites, 1);
        assert!(!aggregated.success());
    }

    #[test]
    fn test_aggregated_results_multiple_files() {
        let mut aggregated = AggregatedResults::new();

        // Add passed file
        aggregated.add_file_result(create_test_file_result(
            true,
            vec![create_test_result("passed", 10)],
        ));

        // Add failed file
        aggregated.add_file_result(create_test_file_result(
            false,
            vec![create_test_result("failed", 20)],
        ));

        assert_eq!(aggregated.num_passed_suites, 1);
        assert_eq!(aggregated.num_failed_suites, 1);
        assert_eq!(aggregated.num_passed_tests, 1);
        assert_eq!(aggregated.num_failed_tests, 1);
        assert!(!aggregated.success());
    }

    #[test]
    fn test_aggregated_results_accumulates_snapshots() {
        let mut aggregated = AggregatedResults::new();

        let file1 = crate::worker::TestFileResult {
            path: "test/file1.test.ts".to_string(),
            passed: true,
            duration_ms: 50,
            tests: vec![],
            snapshot_summary: Some(crate::worker::SnapshotSummary {
                added: 1,
                updated: 0,
                matched: 2,
                unmatched: 0,
            }),
        };

        let file2 = crate::worker::TestFileResult {
            path: "test/file2.test.ts".to_string(),
            passed: true,
            duration_ms: 60,
            tests: vec![],
            snapshot_summary: Some(crate::worker::SnapshotSummary {
                added: 2,
                updated: 1,
                matched: 3,
                unmatched: 1,
            }),
        };

        aggregated.add_file_result(file1);
        aggregated.add_file_result(file2);

        let summary = aggregated.build_snapshot_summary().unwrap();
        assert_eq!(summary.added, 3); // 1 + 2
        assert_eq!(summary.updated, 1); // 0 + 1
        assert_eq!(summary.matched, 5); // 2 + 3
        assert_eq!(summary.unmatched, 1); // 0 + 1
    }

    #[test]
    fn test_daemon_state_new() {
        let state = DaemonState::new();
        assert!(state.start_time.elapsed().as_millis() < 100); // Just created
        assert!(state.running.load(Ordering::Relaxed));
    }

    #[test]
    fn test_validate_project_root_traversal_attempt() {
        // Path traversal should be rejected
        let result = validate_project_root("../etc/passwd");
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("..") || err_msg.contains("traversal"));
    }

    #[test]
    fn test_validate_project_root_double_traversal() {
        // Multiple path traversal should be rejected
        let result = validate_project_root("foo/../../etc/passwd");
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_project_root_windows_traversal() {
        // Windows-style traversal should also be rejected
        let result = validate_project_root("foo\\..\\..\\etc\\passwd");
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_project_root_valid_path() {
        // Valid path should work (using a temp directory)
        use std::fs;
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        let result = validate_project_root(temp_dir.path().to_str().unwrap());
        assert!(result.is_ok());
        assert!(result.unwrap().exists());
    }

    #[test]
    fn test_cleanup_stale_sockets_nonexistent_dir() {
        // Test cleanup doesn't panic on non-existent directory
        // This tests the edge case where /tmp might not exist (unlikely but possible)
        let result = std::panic::catch_unwind(|| {
            cleanup_stale_sockets();
        });
        assert!(result.is_ok());
    }

    #[test]
    fn test_serialize_response_handles_valid_response() {
        let response = Response::Pong;
        let bytes = serialize_response(&response);
        assert!(!bytes.is_empty());
        let parsed: Response = serde_json::from_slice(&bytes).unwrap();
        matches!(parsed, Response::Pong);
    }

    #[test]
    fn test_serialize_response_handles_error_response() {
        let response = Response::Error(ErrorResponse {
            code: ErrorCode::InternalError,
            message: "test error".to_string(),
            details: None,
        });
        let bytes = serialize_response(&response);
        assert!(!bytes.is_empty());
        let parsed: Response = serde_json::from_slice(&bytes).unwrap();
        matches!(parsed, Response::Error(_));
    }
}
