use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};
#[allow(unused_imports)]
use tracing::{debug, info, warn};

use crate::transform::TransformResult;
use rjest_protocol::WorkerHealth;

/// Maximum number of tests a worker can run before being recycled
const MAX_TESTS_PER_WORKER: u64 = 1000;

/// Default maximum number of workers in the pool
const DEFAULT_MAX_WORKERS: usize = 4;

/// Get the configured maximum number of workers from environment variable
/// or return the default value
fn get_max_workers_from_env() -> usize {
    std::env::var("RJEST_MAX_WORKERS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(DEFAULT_MAX_WORKERS)
}

/// How long a worker can be idle before being killed (60 seconds)
const WORKER_IDLE_TIMEOUT: Duration = Duration::from_secs(60);

/// Request to run a test file
#[derive(Debug, Serialize)]
struct RunRequest {
    #[serde(rename = "type")]
    req_type: String,
    path: String,
    code: String,
    config: WorkerConfig,
}

/// Worker configuration passed to each test run
#[derive(Debug, Clone, Serialize)]
pub struct WorkerConfig {
    pub root_dir: PathBuf,
    pub setup_files: Vec<PathBuf>,
    pub setup_files_after_env: Vec<PathBuf>,
    pub test_timeout: u32,
    pub clear_mocks: bool,
    pub reset_mocks: bool,
    pub restore_mocks: bool,
    pub update_snapshots: bool,
    /// Regex pattern to filter test names
    #[serde(skip_serializing_if = "Option::is_none")]
    pub test_name_pattern: Option<String>,
}

/// Result from running a test file
#[derive(Debug, Clone, Deserialize)]
pub struct TestFileResult {
    pub path: String,
    pub passed: bool,
    pub duration_ms: u64,
    pub tests: Vec<TestResult>,
    pub snapshot_summary: Option<SnapshotSummary>,
}

/// Snapshot summary from a test file
#[derive(Debug, Clone, Deserialize, Default)]
pub struct SnapshotSummary {
    pub added: u32,
    pub updated: u32,
    pub matched: u32,
    pub unmatched: u32,
}

/// Result from a single test
#[derive(Debug, Clone, Deserialize)]
pub struct TestResult {
    pub name: String,
    pub status: String,
    pub duration_ms: u64,
    pub error: Option<TestError>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TestError {
    pub message: String,
    pub stack: Option<String>,
    pub diff: Option<String>,
}

/// Job sent to a worker thread
struct WorkerJob {
    index: usize,
    transform: Arc<TransformResult>,
    config: Arc<WorkerConfig>,
}

/// Result from a worker thread
struct WorkerResult {
    index: usize,
    result: Result<TestFileResult>,
}

/// A single worker process running in its own thread
struct Worker {
    process: Child,
    /// Number of tests this worker has executed
    tests_run: u64,
    /// Worker ID for tracking
    id: u32,
    /// Path to respawn
    worker_script: PathBuf,
    /// Last time this worker was used
    last_activity: Instant,
}

impl Worker {
    fn spawn(worker_script: &Path, id: u32) -> Result<Self> {
        // Check if Node.js is available
        let node_path = Command::new("node")
            .arg("--version")
            .output()
            .context("Failed to execute Node.js. Is Node.js installed and in PATH?")?;

        if !node_path.status.success() {
            anyhow::bail!(
                "Node.js is installed but returned non-zero exit code: {}",
                node_path.status
            );
        }

        let process = Command::new("node")
            .arg(worker_script)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .context(format!("Failed to spawn worker process (node version: {})",
                String::from_utf8_lossy(&node_path.stdout).trim()))?;

        Ok(Self {
            process,
            tests_run: 0,
            id,
            worker_script: worker_script.to_path_buf(),
            last_activity: Instant::now(),
        })
    }

    /// Check if this worker has been idle too long
    fn is_idle(&self) -> bool {
        self.last_activity.elapsed() > WORKER_IDLE_TIMEOUT
    }

    /// Check if this worker should be recycled
    fn needs_recycle(&self) -> bool {
        self.tests_run >= MAX_TESTS_PER_WORKER
    }

    fn is_alive(&mut self) -> bool {
        match self.process.try_wait() {
            Ok(Some(_)) => false,
            Ok(None) => true,
            Err(_) => false,
        }
    }

    /// Respawn this worker if dead or needs recycling
    fn ensure_alive(&mut self) -> Result<()> {
        if !self.is_alive() || self.needs_recycle() {
            self.kill();
            let new_process = Command::new("node")
                .arg(&self.worker_script)
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
                .context("Failed to respawn worker process")?;
            self.process = new_process;
            self.tests_run = 0;
            debug!("Respawned worker {}", self.id);
        }
        Ok(())
    }

    fn run_test(&mut self, transform: &TransformResult, config: &WorkerConfig) -> Result<TestFileResult> {
        // Ensure worker is alive before running
        self.ensure_alive()?;

        let request = RunRequest {
            req_type: "run".to_string(),
            path: transform.original_path.to_string_lossy().to_string(),
            code: transform.code.clone(),
            config: config.clone(),
        };

        // Send request
        let stdin = self.process.stdin.as_mut().context("No stdin")?;
        let request_json = serde_json::to_string(&request)?;
        writeln!(stdin, "{}", request_json)?;
        stdin.flush()?;

        // Read response
        let stdout = self.process.stdout.as_mut().context("No stdout")?;
        let mut reader = BufReader::new(stdout);
        let mut line = String::new();
        reader.read_line(&mut line)?;

        self.tests_run += 1;
        self.last_activity = Instant::now();

        // Parse response
        let response: serde_json::Value = serde_json::from_str(&line)?;

        if response["type"] == "error" {
            anyhow::bail!(
                "Worker error: {}",
                response["message"].as_str().unwrap_or("Unknown error")
            );
        }

        let result: TestFileResult = serde_json::from_value(response)?;
        Ok(result)
    }

    /// Gracefully terminate the worker process
    /// First tries SIGTERM, then SIGKILL if it doesn't respond
    fn kill(&mut self) {
        // Try graceful shutdown with SIGTERM first
        let pid = self.process.id();

        // Send SIGTERM
        unsafe {
            libc::kill(pid as libc::pid_t, libc::SIGTERM);
        }

        // Wait up to 2 seconds for graceful shutdown
        let timeout = std::time::Duration::from_secs(2);
        let start = std::time::Instant::now();

        loop {
            match self.process.try_wait() {
                Ok(Some(_)) => {
                    // Process exited gracefully
                    return;
                }
                Ok(None) => {
                    // Process still running
                    if start.elapsed() > timeout {
                        // Timeout - force kill with SIGKILL
                        unsafe {
                            libc::kill(pid as libc::pid_t, libc::SIGKILL);
                        }
                        let _ = self.process.wait();
                        return;
                    }
                    std::thread::sleep(std::time::Duration::from_millis(100));
                }
                Err(_) => {
                    // Error waiting - process is gone
                    return;
                }
            }
        }
    }

    /// Send a warmup request to initialize the worker's Jest runtime
    fn ping(&mut self) -> Result<()> {
        let request = WarmupRequest {
            req_type: "warmup".to_string(),
        };

        let stdin = self.process.stdin.as_mut().context("No stdin")?;
        let request_json = serde_json::to_string(&request)?;
        writeln!(stdin, "{}", request_json)?;
        stdin.flush()?;

        // Read pong response
        let stdout = self.process.stdout.as_mut().context("No stdout")?;
        let mut reader = BufReader::new(stdout);
        let mut line = String::new();
        reader.read_line(&mut line)?;

        Ok(())
    }
}

impl Drop for Worker {
    fn drop(&mut self) {
        self.kill();
    }
}

/// Pool of worker processes with parallel execution support
pub struct WorkerPool {
    /// Workers wrapped in Arc<Mutex> for thread-safe access
    workers: Vec<Arc<Mutex<Worker>>>,
    /// Maximum number of workers allowed (for defensive bounds checking)
    max_workers: usize,
    worker_script: PathBuf,
}

/// Warmup request to pre-initialize workers
#[derive(Debug, Serialize)]
struct WarmupRequest {
    #[serde(rename = "type")]
    req_type: String,
}

impl WorkerPool {
    /// Create a new worker pool (workers are pre-spawned and warmed up)
    pub fn new(max_workers: usize, worker_script: PathBuf) -> Result<Self> {
        // Get the configured max from environment (RJEST_MAX_WORKERS)
        let env_max = get_max_workers_from_env();

        // Limit workers to a reasonable number - more workers = more memory and warmup time
        // The limit is the minimum of: requested workers, environment config, and a reasonable hard cap
        let hard_cap = 16; // Prevent runaway memory usage
        let effective_workers = max_workers.min(env_max).min(hard_cap);
        info!("Creating worker pool with {} workers (requested: {}, env limit: {})", effective_workers, max_workers, env_max);

        let mut workers = Vec::with_capacity(effective_workers);

        // Pre-spawn workers
        for id in 0..effective_workers {
            match Worker::spawn(&worker_script, id as u32) {
                Ok(worker) => workers.push(Arc::new(Mutex::new(worker))),
                Err(e) => warn!("Failed to spawn worker: {}", e),
            }
        }

        info!("Spawned {} workers", workers.len());

        let pool = Self {
            workers,
            max_workers: effective_workers,
            worker_script,
        };

        // Warm up all workers in parallel
        pool.warmup_workers();

        Ok(pool)
    }

    /// Send ping to all workers to warm them up
    fn warmup_workers(&self) {
        use std::thread;

        let handles: Vec<_> = self.workers.iter().map(|worker_arc| {
            let worker = Arc::clone(worker_arc);
            thread::spawn(move || {
                if let Ok(mut w) = worker.lock() {
                    let _ = w.ping();
                }
            })
        }).collect();

        for handle in handles {
            if let Err(e) = handle.join() {
                warn!("Worker warmup thread panicked: {:?}", e);
            }
        }

        debug!("All workers warmed up");
    }

    /// Remove idle workers to free memory
    pub fn cleanup_idle_workers(&mut self) {
        let initial_count = self.workers.len();

        // Keep at least 1 worker, remove idle ones
        self.workers.retain(|worker_arc| {
            if let Ok(worker) = worker_arc.lock() {
                // Keep if not idle or if it's the last worker
                !worker.is_idle()
            } else {
                false // Remove if can't lock (shouldn't happen)
            }
        });

        // Ensure at least 1 worker remains (but don't exceed max_workers)
        if self.workers.is_empty() && initial_count > 0 && self.workers.len() < self.max_workers {
            if let Ok(worker) = Worker::spawn(&self.worker_script, 0) {
                self.workers.push(Arc::new(Mutex::new(worker)));
            }
        }

        let removed = initial_count.saturating_sub(self.workers.len());
        if removed > 0 {
            info!("Cleaned up {} idle workers, {} remaining (max: {})", removed, self.workers.len(), self.max_workers);
        }
    }

    /// Get current number of workers
    pub fn worker_count(&self) -> usize {
        self.workers.len()
    }

    /// Get maximum number of workers allowed
    pub fn max_workers(&self) -> usize {
        self.max_workers
    }

    /// Run a single test file (for compatibility)
    pub fn run_test(&mut self, transform: &TransformResult, config: &WorkerConfig) -> Result<TestFileResult> {
        debug!("Running test {} in worker", transform.original_path.display());

        // Use the first available worker
        if let Some(worker_arc) = self.workers.first() {
            let mut worker = worker_arc.lock().map_err(|e| anyhow::anyhow!("Worker lock poisoned: {}", e))?;
            worker.run_test(transform, config)
        } else {
            anyhow::bail!("No workers available")
        }
    }

    /// Run multiple test files in parallel across all workers
    pub fn run_tests(&mut self, transforms: &[TransformResult], config: &WorkerConfig) -> Vec<Result<TestFileResult>> {
        if transforms.is_empty() {
            return vec![];
        }

        let num_tests = transforms.len();
        // Only use as many workers as we have tests to ensure consistent warmup
        let num_workers = self.workers.len().min(num_tests).max(1);

        // For small number of tests, just run sequentially to avoid thread overhead
        if num_tests <= 1 || num_workers <= 1 {
            return transforms.iter().map(|t| self.run_test(t, config)).collect();
        }

        info!("Running {} tests in parallel across {} workers", num_tests, num_workers);

        // Create channels for job distribution and result collection
        let (job_tx, job_rx): (Sender<WorkerJob>, Receiver<WorkerJob>) = channel();
        let (result_tx, result_rx): (Sender<WorkerResult>, Receiver<WorkerResult>) = channel();

        // Wrap receiver in Arc<Mutex> for sharing between threads
        let job_rx = Arc::new(Mutex::new(job_rx));

        // Spawn worker threads (only for the workers we'll actually use)
        let mut handles: Vec<JoinHandle<()>> = Vec::with_capacity(num_workers);

        for worker_arc in self.workers.iter().take(num_workers) {
            let worker = Arc::clone(worker_arc);
            let job_rx = Arc::clone(&job_rx);
            let result_tx = result_tx.clone();

            let handle = thread::spawn(move || {
                loop {
                    // Try to get a job from the queue
                    let job = {
                        let rx = match job_rx.lock() {
                            Ok(rx) => rx,
                            Err(e) => {
                                let _ = result_tx.send(WorkerResult {
                                    index: 0,
                                    result: Err(anyhow::anyhow!("Job receiver lock poisoned: {}", e)),
                                });
                                break;
                            }
                        };
                        rx.recv()
                    };

                    match job {
                        Ok(job) => {
                            // Run the test (dereference Arc to get references)
                            let result = match worker.lock() {
                                Ok(mut w) => w.run_test(&*job.transform, &*job.config),
                                Err(e) => Err(anyhow::anyhow!("Worker lock poisoned: {}", e)),
                            };

                            // Send result back
                            let _ = result_tx.send(WorkerResult {
                                index: job.index,
                                result,
                            });
                        }
                        Err(_) => {
                            // Channel closed, exit thread
                            break;
                        }
                    }
                }
            });

            handles.push(handle);
        }

        // Drop the extra result sender so we can detect when all workers are done
        drop(result_tx);

        // Pre-wrap config in Arc (shared across all jobs)
        let config = Arc::new(config.clone());

        // Send all jobs
        for (index, transform) in transforms.iter().enumerate() {
            let job = WorkerJob {
                index,
                transform: Arc::new(transform.clone()),
                config: Arc::clone(&config),
            };
            if job_tx.send(job).is_err() {
                warn!("Failed to send job {}", index);
            }
        }

        // Close the job channel to signal workers to exit after processing all jobs
        drop(job_tx);

        // Collect results
        let mut results: Vec<Option<Result<TestFileResult>>> = (0..num_tests).map(|_| None).collect();
        for worker_result in result_rx {
            results[worker_result.index] = Some(worker_result.result);
        }

        // Wait for all worker threads to finish
        let mut worker_panicked = false;
        for handle in handles {
            if let Err(e) = handle.join() {
                warn!("Worker thread panicked: {:?}", e);
                worker_panicked = true;
            }
        }

        // If any worker panicked, add error to any missing results
        if worker_panicked {
            for result in &mut results {
                if result.is_none() {
                    *result = Some(Err(anyhow::anyhow!("Worker thread panicked")));
                }
            }
        }

        // Convert to final result vector
        results
            .into_iter()
            .enumerate()
            .map(|(i, r)| r.unwrap_or_else(|| Err(anyhow::anyhow!("No result for test {}", i))))
            .collect()
    }

    /// Shutdown all workers
    pub fn shutdown(&mut self) {
        info!("Shutting down worker pool");
        for worker_arc in &self.workers {
            if let Ok(mut worker) = worker_arc.lock() {
                worker.kill();
            }
        }
        self.workers.clear();
    }

    /// Get health status of all workers
    pub fn health(&self) -> Vec<WorkerHealth> {
        self.workers.iter().filter_map(|worker_arc| {
            match worker_arc.lock() {
                Ok(mut worker) => Some(WorkerHealth {
                    id: worker.id,
                    alive: worker.is_alive(),
                    busy: false, // We don't track busy state currently
                    tests_run: worker.tests_run,
                    idle_secs: worker.last_activity.elapsed().as_secs(),
                }),
                Err(_) => None,
            }
        }).collect()
    }
}

impl Drop for WorkerPool {
    fn drop(&mut self) {
        self.shutdown();
    }
}

/// Find the worker script bundled with jestd
pub fn find_worker_script() -> Result<PathBuf> {
    // Check relative to executable
    if let Ok(exe) = std::env::current_exe() {
        if let Some(exe_dir) = exe.parent() {
            let candidates = [
                exe_dir.join("../lib/rjest-runtime/src/worker.js"),
                exe_dir.join("../../crates/rjest-runtime/src/worker.js"),
                exe_dir.join("worker.js"),
            ];

            for candidate in candidates {
                if candidate.exists() {
                    return Ok(candidate.canonicalize()?);
                }
            }
        }
    }

    // Fallback for development
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let dev_path = PathBuf::from(manifest_dir).join("../rjest-runtime/src/worker.js");
    if dev_path.exists() {
        return Ok(dev_path.canonicalize()?);
    }

    anyhow::bail!("Could not find worker script")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_worker_config_default_values() {
        let config = WorkerConfig {
            root_dir: PathBuf::from("/test"),
            setup_files: vec![],
            setup_files_after_env: vec![],
            test_timeout: 5000,
            clear_mocks: false,
            reset_mocks: false,
            restore_mocks: false,
            update_snapshots: false,
            test_name_pattern: None,
        };

        assert_eq!(config.test_timeout, 5000);
        assert!(!config.clear_mocks);
        assert!(!config.update_snapshots);
    }

    #[test]
    fn test_test_result_creation() {
        let result = TestResult {
            name: "test example".to_string(),
            status: "passed".to_string(),
            duration_ms: 100,
            error: None,
        };

        assert_eq!(result.name, "test example");
        assert_eq!(result.status, "passed");
        assert_eq!(result.duration_ms, 100);
        assert!(result.error.is_none());
    }

    #[test]
    fn test_test_file_result_creation() {
        let result = TestFileResult {
            path: "test/example.test.ts".to_string(),
            passed: true,
            duration_ms: 500,
            tests: vec![],
            snapshot_summary: None,
        };

        assert!(result.passed);
        assert_eq!(result.duration_ms, 500);
        assert!(result.tests.is_empty());
        assert!(result.snapshot_summary.is_none());
    }

    #[test]
    fn test_snapshot_summary_default() {
        let summary = SnapshotSummary::default();
        assert_eq!(summary.added, 0);
        assert_eq!(summary.updated, 0);
        assert_eq!(summary.matched, 0);
        assert_eq!(summary.unmatched, 0);
    }

    #[test]
    fn test_snapshot_summary_with_values() {
        let summary = SnapshotSummary {
            added: 2,
            updated: 1,
            matched: 5,
            unmatched: 1,
        };

        assert_eq!(summary.added, 2);
        assert_eq!(summary.updated, 1);
        assert_eq!(summary.matched, 5);
        assert_eq!(summary.unmatched, 1);
    }

    #[test]
    fn test_test_error_creation() {
        let error = TestError {
            message: "Expected 1 to equal 2".to_string(),
            stack: Some("at test (test/example.test.ts:10:5)".to_string()),
            diff: Some("  - Expected\n  + Received\n\n  1\n  2".to_string()),
        };

        assert!(error.message.contains("Expected"));
        assert!(error.stack.is_some());
        assert!(error.diff.is_some());
    }

    #[test]
    fn test_worker_config_with_test_pattern() {
        let config = WorkerConfig {
            root_dir: PathBuf::from("/test"),
            setup_files: vec![],
            setup_files_after_env: vec![],
            test_timeout: 5000,
            clear_mocks: false,
            reset_mocks: false,
            restore_mocks: false,
            update_snapshots: false,
            test_name_pattern: Some("skip.*".to_string()),
        };

        assert_eq!(config.test_name_pattern, Some("skip.*".to_string()));
    }
}
