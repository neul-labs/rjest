use criterion::{black_box, criterion_group, criterion_main, Criterion};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::Duration;

fn project_root() -> PathBuf {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    PathBuf::from(manifest_dir)
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf()
}

fn run_rjest_warm(fixture_path: &PathBuf) {
    let root = project_root();
    let jest_bin = root.join("target/release/jest");

    let _output = Command::new(&jest_bin)
        .current_dir(fixture_path)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .expect("Failed to run rjest");
}

fn stop_daemon() {
    let root = project_root();
    let jest_bin = root.join("target/release/jest");

    let _ = Command::new(&jest_bin)
        .arg("--daemon-stop")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .output();

    std::thread::sleep(Duration::from_millis(500));
}

fn benchmark_warm_run(c: &mut Criterion) {
    let root = project_root();
    let fixture = root.join("tests/fixtures/basic-ts");

    // Ensure daemon is running with a warmup run
    run_rjest_warm(&fixture);

    c.bench_function("warm_run_basic_ts", |b| {
        b.iter(|| run_rjest_warm(black_box(&fixture)))
    });
}

fn benchmark_cold_start(c: &mut Criterion) {
    let root = project_root();
    let fixture = root.join("tests/fixtures/basic-ts");
    let jest_bin = root.join("target/release/jest");

    let mut group = c.benchmark_group("cold_start");
    group.sample_size(10); // Fewer samples for cold start (slower)

    group.bench_function("cold_start_basic_ts", |b| {
        b.iter(|| {
            // Stop daemon
            let _ = Command::new(&jest_bin)
                .arg("--daemon-stop")
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .output();
            std::thread::sleep(Duration::from_millis(300));

            // Run cold
            let _output = Command::new(&jest_bin)
                .current_dir(&fixture)
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .output()
                .expect("Failed to run rjest");
        })
    });

    group.finish();

    // Clean up
    stop_daemon();
}

criterion_group!(benches, benchmark_warm_run, benchmark_cold_start);
criterion_main!(benches);
