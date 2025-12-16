//! Benchmark utilities for rjest

use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// Result of a benchmark run
#[derive(Debug, Clone)]
pub struct BenchResult {
    pub name: String,
    pub iterations: u32,
    pub total_time: Duration,
    pub avg_time: Duration,
    pub min_time: Duration,
    pub max_time: Duration,
}

/// Run a benchmark and collect statistics
pub fn run_benchmark<F>(name: &str, iterations: u32, mut f: F) -> BenchResult
where
    F: FnMut(),
{
    let mut times = Vec::with_capacity(iterations as usize);

    for _ in 0..iterations {
        let start = Instant::now();
        f();
        times.push(start.elapsed());
    }

    let total: Duration = times.iter().sum();
    let avg = total / iterations;
    let min = *times.iter().min().unwrap();
    let max = *times.iter().max().unwrap();

    BenchResult {
        name: name.to_string(),
        iterations,
        total_time: total,
        avg_time: avg,
        min_time: min,
        max_time: max,
    }
}

/// Find the project root directory
pub fn project_root() -> PathBuf {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    PathBuf::from(manifest_dir)
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf()
}

/// Run rjest CLI and return execution time
pub fn run_rjest(fixture_path: &PathBuf) -> Duration {
    let root = project_root();
    let jest_bin = root.join("target/release/jest");

    let start = Instant::now();
    let _output = Command::new(&jest_bin)
        .current_dir(fixture_path)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .expect("Failed to run rjest");
    start.elapsed()
}

/// Run upstream Jest and return execution time
pub fn run_jest(fixture_path: &PathBuf) -> Option<Duration> {
    let start = Instant::now();
    let output = Command::new("npx")
        .arg("jest")
        .current_dir(fixture_path)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .output();

    match output {
        Ok(_) => Some(start.elapsed()),
        Err(_) => None,
    }
}

/// Stop the rjest daemon
pub fn stop_daemon() {
    let root = project_root();
    let jest_bin = root.join("target/release/jest");

    let _ = Command::new(&jest_bin)
        .arg("--daemon-stop")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .output();

    std::thread::sleep(Duration::from_millis(500));
}

impl std::fmt::Display for BenchResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}: avg={:?}, min={:?}, max={:?} ({} iterations)",
            self.name, self.avg_time, self.min_time, self.max_time, self.iterations
        )
    }
}
