use criterion::{black_box, criterion_group, criterion_main, Criterion};
use std::path::PathBuf;
use std::process::{Command, Stdio};

fn project_root() -> PathBuf {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    PathBuf::from(manifest_dir)
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf()
}

fn run_rjest_pattern(fixture_path: &PathBuf, pattern: &str) {
    let root = project_root();
    let jest_bin = root.join("target/release/jest");

    let _output = Command::new(&jest_bin)
        .arg(pattern)
        .current_dir(fixture_path)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .expect("Failed to run rjest");
}

fn benchmark_pattern_matching(c: &mut Criterion) {
    let root = project_root();
    let fixture = root.join("tests/fixtures/basic-ts");

    // Warmup
    run_rjest_pattern(&fixture, "utils");

    let mut group = c.benchmark_group("pattern_matching");

    group.bench_function("exact_file_pattern", |b| {
        b.iter(|| run_rjest_pattern(black_box(&fixture), black_box("utils.test.ts")))
    });

    group.bench_function("glob_pattern", |b| {
        b.iter(|| run_rjest_pattern(black_box(&fixture), black_box("*.test.ts")))
    });

    group.bench_function("substring_pattern", |b| {
        b.iter(|| run_rjest_pattern(black_box(&fixture), black_box("util")))
    });

    group.finish();
}

fn benchmark_monorepo(c: &mut Criterion) {
    let root = project_root();
    let fixture = root.join("tests/fixtures/monorepo");

    // Warmup
    let jest_bin = root.join("target/release/jest");
    let _ = Command::new(&jest_bin)
        .current_dir(&fixture)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .output();

    c.bench_function("monorepo_all_projects", |b| {
        b.iter(|| {
            let _output = Command::new(&jest_bin)
                .current_dir(black_box(&fixture))
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .output()
                .expect("Failed to run rjest");
        })
    });
}

criterion_group!(benches, benchmark_pattern_matching, benchmark_monorepo);
criterion_main!(benches);
