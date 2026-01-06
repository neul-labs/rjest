//! Metrics collection for rjest daemon
//!
//! Tracks various performance and operational metrics:
//! - Test execution timing
//! - Transform cache hit/miss rates
//! - Worker utilization
//! - Request latencies

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::OnceLock;
use std::time::Instant;
use tracing::{debug, info};

/// Global metrics instance
static METRICS: OnceLock<Metrics> = OnceLock::new();

/// Daemon metrics
pub struct Metrics {
    /// Total test files executed
    pub total_test_files: AtomicU64,
    /// Total individual tests executed
    pub total_tests: AtomicU64,
    /// Total passed tests
    pub passed_tests: AtomicU64,
    /// Total failed tests
    pub failed_tests: AtomicU64,
    /// Transform cache hits
    pub cache_hits: AtomicU64,
    /// Transform cache misses
    pub cache_misses: AtomicU64,
    /// Total requests handled
    pub requests_handled: AtomicU64,
    /// Total time spent in test execution (microseconds)
    pub total_test_time_us: AtomicU64,
    /// Total time spent in transforms (microseconds)
    pub total_transform_time_us: AtomicU64,
}

impl Metrics {
    fn new() -> Self {
        Self {
            total_test_files: AtomicU64::new(0),
            total_tests: AtomicU64::new(0),
            passed_tests: AtomicU64::new(0),
            failed_tests: AtomicU64::new(0),
            cache_hits: AtomicU64::new(0),
            cache_misses: AtomicU64::new(0),
            requests_handled: AtomicU64::new(0),
            total_test_time_us: AtomicU64::new(0),
            total_transform_time_us: AtomicU64::new(0),
        }
    }

    /// Get a snapshot of current metrics
    pub fn snapshot(&self) -> MetricsSnapshot {
        MetricsSnapshot {
            total_test_files: self.total_test_files.load(Ordering::Relaxed),
            total_tests: self.total_tests.load(Ordering::Relaxed),
            passed_tests: self.passed_tests.load(Ordering::Relaxed),
            failed_tests: self.failed_tests.load(Ordering::Relaxed),
            cache_hits: self.cache_hits.load(Ordering::Relaxed),
            cache_misses: self.cache_misses.load(Ordering::Relaxed),
            requests_handled: self.requests_handled.load(Ordering::Relaxed),
            total_test_time_us: self.total_test_time_us.load(Ordering::Relaxed),
            total_transform_time_us: self.total_transform_time_us.load(Ordering::Relaxed),
        }
    }
}

/// Snapshot of metrics at a point in time
#[derive(Debug, Clone)]
pub struct MetricsSnapshot {
    pub total_test_files: u64,
    pub total_tests: u64,
    pub passed_tests: u64,
    pub failed_tests: u64,
    pub cache_hits: u64,
    pub cache_misses: u64,
    pub requests_handled: u64,
    pub total_test_time_us: u64,
    pub total_transform_time_us: u64,
}

impl MetricsSnapshot {
    /// Get total number of transforms (cache hits + misses)
    pub fn transform_count(&self) -> u64 {
        self.cache_hits + self.cache_misses
    }
}

impl MetricsSnapshot {
    /// Calculate cache hit rate (0.0 - 1.0)
    pub fn cache_hit_rate(&self) -> f64 {
        let total = self.cache_hits + self.cache_misses;
        if total == 0 {
            0.0
        } else {
            self.cache_hits as f64 / total as f64
        }
    }

    /// Calculate test pass rate (0.0 - 1.0)
    pub fn pass_rate(&self) -> f64 {
        let total = self.passed_tests + self.failed_tests;
        if total == 0 {
            1.0
        } else {
            self.passed_tests as f64 / total as f64
        }
    }

    /// Average test time in milliseconds
    pub fn avg_test_time_ms(&self) -> f64 {
        if self.total_test_files == 0 {
            0.0
        } else {
            (self.total_test_time_us as f64 / 1000.0) / self.total_test_files as f64
        }
    }

    /// Average transform time in milliseconds
    pub fn avg_transform_time_ms(&self) -> f64 {
        let total_transforms = self.cache_hits + self.cache_misses;
        if total_transforms == 0 {
            0.0
        } else {
            (self.total_transform_time_us as f64 / 1000.0) / total_transforms as f64
        }
    }
}

/// Initialize global metrics
pub fn init() {
    METRICS.get_or_init(Metrics::new);
    info!("Metrics initialized");
}

/// Get global metrics instance
pub fn get() -> &'static Metrics {
    METRICS.get_or_init(Metrics::new)
}

/// Record a test file execution
pub fn record_test_file(duration_us: u64) {
    let m = get();
    m.total_test_files.fetch_add(1, Ordering::Relaxed);
    m.total_test_time_us.fetch_add(duration_us, Ordering::Relaxed);
}

/// Record individual test results
pub fn record_test_results(passed: u64, failed: u64) {
    let m = get();
    m.total_tests.fetch_add(passed + failed, Ordering::Relaxed);
    m.passed_tests.fetch_add(passed, Ordering::Relaxed);
    m.failed_tests.fetch_add(failed, Ordering::Relaxed);
}

/// Record a transform cache hit
pub fn record_cache_hit() {
    get().cache_hits.fetch_add(1, Ordering::Relaxed);
}

/// Record a transform cache miss
pub fn record_cache_miss() {
    get().cache_misses.fetch_add(1, Ordering::Relaxed);
}

/// Record a transform operation
pub fn record_transform(duration_us: u64, hit: bool) {
    let m = get();
    m.total_transform_time_us.fetch_add(duration_us, Ordering::Relaxed);
    if hit {
        m.cache_hits.fetch_add(1, Ordering::Relaxed);
    } else {
        m.cache_misses.fetch_add(1, Ordering::Relaxed);
    }
}

/// Record a request handled
pub fn record_request() {
    get().requests_handled.fetch_add(1, Ordering::Relaxed);
}

/// Get a metrics snapshot
pub fn snapshot() -> MetricsSnapshot {
    get().snapshot()
}

/// Log current metrics summary
pub fn log_summary() {
    let s = snapshot();
    debug!(
        "Metrics: requests={} tests={} ({}% pass) cache_hit_rate={:.1}% avg_test={:.1}ms",
        s.requests_handled,
        s.total_tests,
        (s.pass_rate() * 100.0) as u32,
        s.cache_hit_rate() * 100.0,
        s.avg_test_time_ms()
    );
}

/// RAII guard for timing operations
pub struct TimingGuard {
    start: Instant,
    name: &'static str,
}

impl TimingGuard {
    pub fn new(name: &'static str) -> Self {
        Self {
            start: Instant::now(),
            name,
        }
    }

    pub fn elapsed_us(&self) -> u64 {
        self.start.elapsed().as_micros() as u64
    }
}

impl Drop for TimingGuard {
    fn drop(&mut self) {
        let elapsed = self.start.elapsed();
        debug!("{} completed in {:?}", self.name, elapsed);
    }
}

/// Create a timing guard for an operation
#[macro_export]
macro_rules! time_operation {
    ($name:expr) => {
        $crate::metrics::TimingGuard::new($name)
    };
}
