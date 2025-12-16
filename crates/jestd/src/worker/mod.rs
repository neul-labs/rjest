use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::Instant;
use tracing::{debug, info, warn};

use crate::transform::TransformResult;

/// Maximum number of tests a worker can run before being recycled
const MAX_TESTS_PER_WORKER: u64 = 1000;

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

/// A single worker process
struct Worker {
    process: Child,
    busy: bool,
    /// Number of tests this worker has executed
    tests_run: u64,
    /// Time of last activity
    last_activity: Instant,
    /// Worker ID for tracking
    id: u32,
}

impl Worker {
    fn spawn(worker_script: &Path, id: u32) -> Result<Self> {
        let process = Command::new("node")
            .arg(worker_script)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .context("Failed to spawn worker process")?;

        Ok(Self {
            process,
            busy: false,
            tests_run: 0,
            last_activity: Instant::now(),
            id,
        })
    }

    /// Check if this worker should be recycled
    fn needs_recycle(&self) -> bool {
        self.tests_run >= MAX_TESTS_PER_WORKER
    }

    fn is_alive(&mut self) -> bool {
        match self.process.try_wait() {
            Ok(Some(_)) => false, // Process has exited
            Ok(None) => true,     // Still running
            Err(_) => false,      // Error checking
        }
    }

    fn run_test(&mut self, transform: &TransformResult, config: &WorkerConfig) -> Result<TestFileResult> {
        self.busy = true;
        self.last_activity = Instant::now();

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

        self.busy = false;
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

    fn kill(&mut self) {
        let _ = self.process.kill();
    }
}

impl Drop for Worker {
    fn drop(&mut self) {
        self.kill();
    }
}

/// Pool of worker processes
pub struct WorkerPool {
    workers: Vec<Worker>,
    worker_script: PathBuf,
    max_workers: usize,
    config: WorkerConfig,
    /// Next worker ID to assign
    next_worker_id: u32,
}

impl WorkerPool {
    /// Create a new worker pool
    pub fn new(max_workers: usize, worker_script: PathBuf, config: WorkerConfig) -> Result<Self> {
        info!("Creating worker pool with {} workers", max_workers);

        let mut pool = Self {
            workers: Vec::with_capacity(max_workers),
            worker_script,
            max_workers,
            config,
            next_worker_id: 0,
        };

        // Pre-spawn workers
        for _ in 0..max_workers {
            let id = pool.next_worker_id;
            pool.next_worker_id += 1;
            match Worker::spawn(&pool.worker_script, id) {
                Ok(worker) => pool.workers.push(worker),
                Err(e) => warn!("Failed to spawn worker: {}", e),
            }
        }

        info!("Spawned {} workers", pool.workers.len());
        Ok(pool)
    }

    /// Run a test file in an available worker
    pub fn run_test(&mut self, transform: &TransformResult) -> Result<TestFileResult> {
        debug!("Running test {} in worker", transform.original_path.display());

        // Clone config to avoid borrow issues
        let config = self.config.clone();

        // Find an available worker or spawn a new one
        let worker = self.get_worker()?;

        worker.run_test(transform, &config)
    }

    /// Run multiple test files, potentially in parallel
    pub fn run_tests(&mut self, transforms: &[TransformResult]) -> Vec<Result<TestFileResult>> {
        // For now, run sequentially
        // TODO: Implement parallel execution with worker distribution
        transforms
            .iter()
            .map(|t| self.run_test(t))
            .collect()
    }

    /// Get an available worker, spawning if necessary
    fn get_worker(&mut self) -> Result<&mut Worker> {
        // First, recycle any workers that have run too many tests
        self.recycle_exhausted_workers();

        // Remove dead workers
        self.workers.retain_mut(|w| w.is_alive());

        // Find an idle, alive worker
        for i in 0..self.workers.len() {
            if !self.workers[i].busy && self.workers[i].is_alive() {
                return Ok(&mut self.workers[i]);
            }
        }

        // Spawn a new worker if under limit
        if self.workers.len() < self.max_workers {
            let id = self.next_worker_id;
            self.next_worker_id += 1;
            let worker = Worker::spawn(&self.worker_script, id)?;
            self.workers.push(worker);
            return Ok(self.workers.last_mut().unwrap());
        }

        // Wait for any worker to become available
        // For now, just use the first one
        if let Some(worker) = self.workers.first_mut() {
            Ok(worker)
        } else {
            anyhow::bail!("No workers available")
        }
    }

    /// Recycle workers that have run too many tests
    fn recycle_exhausted_workers(&mut self) {
        let mut indices_to_recycle = Vec::new();

        for (i, worker) in self.workers.iter().enumerate() {
            if worker.needs_recycle() && !worker.busy {
                indices_to_recycle.push(i);
            }
        }

        // Recycle in reverse order to avoid index shifting
        for i in indices_to_recycle.into_iter().rev() {
            let old_id = self.workers[i].id;
            debug!("Recycling worker {} after {} tests", old_id, self.workers[i].tests_run);
            self.workers[i].kill();
            self.workers.remove(i);

            // Spawn replacement
            let new_id = self.next_worker_id;
            self.next_worker_id += 1;
            match Worker::spawn(&self.worker_script, new_id) {
                Ok(worker) => {
                    debug!("Spawned replacement worker {}", new_id);
                    self.workers.push(worker);
                }
                Err(e) => warn!("Failed to spawn replacement worker: {}", e),
            }
        }
    }

    /// Get worker statistics
    pub fn stats(&self) -> WorkerStats {
        let active = self.workers.iter().filter(|w| w.busy).count() as u32;
        let idle = self.workers.len() as u32 - active;

        WorkerStats {
            active,
            idle,
            max: self.max_workers as u32,
        }
    }

    /// Get detailed health information for all workers
    pub fn health(&self) -> Vec<WorkerHealthInfo> {
        self.workers
            .iter()
            .map(|w| WorkerHealthInfo {
                id: w.id,
                alive: true, // If it's in the list, it was alive last check
                busy: w.busy,
                tests_run: w.tests_run,
                idle_secs: w.last_activity.elapsed().as_secs(),
            })
            .collect()
    }

    /// Get total tests run across all workers
    pub fn total_tests_run(&self) -> u64 {
        self.workers.iter().map(|w| w.tests_run).sum()
    }

    /// Shutdown all workers
    pub fn shutdown(&mut self) {
        info!("Shutting down worker pool");
        for worker in &mut self.workers {
            worker.kill();
        }
        self.workers.clear();
    }
}

impl Drop for WorkerPool {
    fn drop(&mut self) {
        self.shutdown();
    }
}

#[derive(Debug, Clone)]
pub struct WorkerStats {
    pub active: u32,
    pub idle: u32,
    pub max: u32,
}

/// Health information for a single worker
#[derive(Debug, Clone)]
pub struct WorkerHealthInfo {
    pub id: u32,
    pub alive: bool,
    pub busy: bool,
    pub tests_run: u64,
    pub idle_secs: u64,
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
