use anyhow::{Context, Result};
use nng::{Protocol, Socket};
use rjest_protocol::{
    ipc_address, socket_path, ErrorCode, ErrorResponse, Request, Response, RunResponse,
    StatusResponse, TestFileResult as ProtoTestFileResult, TestResult as ProtoTestResult,
    TestStatus, TestError as ProtoTestError, SourceLocation,
    CacheStats as ProtoCacheStats, WorkerStats as ProtoWorkerStats, RunRequest,
};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;
use tracing::{debug, error, info, warn};

use crate::config::JestConfig;
use crate::discovery::TestDiscovery;
use crate::transform::Transformer;
use crate::worker::{find_worker_script, WorkerConfig, WorkerPool};

/// Daemon state shared across requests
struct DaemonState {
    start_time: Instant,
    running: AtomicBool,
    total_tests_run: AtomicU64,
    /// Cached configs per project root
    configs: Mutex<HashMap<PathBuf, JestConfig>>,
    /// Transform cache directory
    cache_dir: PathBuf,
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
    }
}

fn execute_tests(request: &RunRequest, state: &Arc<DaemonState>) -> Result<RunResponse> {
    let start_time = Instant::now();
    let project_root = PathBuf::from(&request.project_root);

    info!("Executing tests for {}", project_root.display());

    // Load configuration
    let config = state.get_or_load_config(&project_root)?;

    // Discover test files
    let discovery = TestDiscovery::new(config.clone());
    let test_files = if !request.flags.find_related_tests.is_empty() {
        let related: Vec<PathBuf> = request.flags.find_related_tests
            .iter()
            .map(PathBuf::from)
            .collect();
        discovery.find_related_tests(&related)?
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

    // Transform test files
    let transforms: Vec<_> = test_files
        .iter()
        .filter_map(|path| {
            match transformer.transform(path) {
                Ok(t) => Some(t),
                Err(e) => {
                    warn!("Failed to transform {}: {}", path.display(), e);
                    None
                }
            }
        })
        .collect();

    // Find worker script
    let worker_script = find_worker_script()?;

    // Create worker pool
    let worker_config = WorkerConfig {
        root_dir: config.root_dir.clone(),
        setup_files: config.setup_files.clone(),
        setup_files_after_env: config.setup_files_after_env.clone(),
        test_timeout: config.test_timeout,
        clear_mocks: config.clear_mocks,
        reset_mocks: config.reset_mocks,
        restore_mocks: config.restore_mocks,
    };

    let max_workers = if request.flags.run_in_band {
        1
    } else {
        request.flags.max_workers.map(|w| w as usize).unwrap_or_else(|| config.max_workers_count())
    };

    let mut pool = WorkerPool::new(max_workers, worker_script, worker_config)?;

    // Run tests
    let results = pool.run_tests(&transforms);

    // Aggregate results
    let mut test_results = Vec::new();
    let mut num_passed_suites = 0u32;
    let mut num_failed_suites = 0u32;
    let mut num_passed_tests = 0u32;
    let mut num_failed_tests = 0u32;
    let mut num_skipped_tests = 0u32;
    let mut num_todo_tests = 0u32;

    for result in results {
        match result {
            Ok(file_result) => {
                if file_result.passed {
                    num_passed_suites += 1;
                } else {
                    num_failed_suites += 1;
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
        snapshot_summary: None,
    })
}
