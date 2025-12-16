use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use tracing::{debug, info, warn};

use crate::transform::TransformResult;

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
}

/// Result from running a test file
#[derive(Debug, Clone, Deserialize)]
pub struct TestFileResult {
    pub path: String,
    pub passed: bool,
    pub duration_ms: u64,
    pub tests: Vec<TestResult>,
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
}

impl Worker {
    fn spawn(worker_script: &Path) -> Result<Self> {
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
        })
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
        };

        // Pre-spawn workers
        for _ in 0..max_workers {
            match Worker::spawn(&pool.worker_script) {
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
        // First, try to find an idle worker
        for (i, worker) in self.workers.iter_mut().enumerate() {
            if !worker.busy && worker.is_alive() {
                return Ok(&mut self.workers[i]);
            }
        }

        // Remove dead workers
        self.workers.retain_mut(|w| w.is_alive());

        // Spawn a new worker if under limit
        if self.workers.len() < self.max_workers {
            let worker = Worker::spawn(&self.worker_script)?;
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
