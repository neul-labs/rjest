use anyhow::Result;
use rjest_protocol::{RunResponse, TestFileResult, TestResult, TestStatus};

use crate::args::Args;

/// Render test results to stdout
pub fn render(response: &RunResponse, args: &Args) -> Result<()> {
    if args.json || args.machine {
        render_json(response)?;
    } else {
        render_human(response, args.verbose)?;
    }
    Ok(())
}

fn render_json(response: &RunResponse) -> Result<()> {
    let json = serde_json::to_string_pretty(response)?;
    println!("{}", json);
    Ok(())
}

fn render_human(response: &RunResponse, verbose: bool) -> Result<()> {
    // Print per-file results
    for file_result in &response.test_results {
        print_file_result(file_result, verbose);
    }

    // Print failures in detail
    let failures: Vec<_> = response
        .test_results
        .iter()
        .flat_map(|f| {
            f.tests
                .iter()
                .filter(|t| t.status == TestStatus::Failed)
                .map(move |t| (&f.path, t))
        })
        .collect();

    if !failures.is_empty() {
        println!();
        println!("Failures:");
        println!();
        for (path, test) in failures {
            print_failure(path, test);
        }
    }

    // Print summary
    println!();
    print_summary(response);

    Ok(())
}

fn print_file_result(result: &TestFileResult, verbose: bool) {
    let status = if result.passed {
        "\x1b[32m PASS \x1b[0m"
    } else {
        "\x1b[31m FAIL \x1b[0m"
    };

    println!("{} {}", status, result.path);

    if verbose {
        for test in &result.tests {
            let (icon, color) = match test.status {
                TestStatus::Passed => ("✓", "\x1b[32m"),
                TestStatus::Failed => ("✕", "\x1b[31m"),
                TestStatus::Skipped => ("○", "\x1b[33m"),
                TestStatus::Todo => ("✎", "\x1b[35m"),
            };
            println!(
                "  {}{} {}\x1b[0m ({} ms)",
                color, icon, test.name, test.duration_ms
            );
        }
    }
}

fn print_failure(path: &str, test: &TestResult) {
    println!("  \x1b[31m● {} > {}\x1b[0m", path, test.name);
    println!();

    if let Some(ref error) = test.error {
        // Print error message
        for line in error.message.lines() {
            println!("    {}", line);
        }
        println!();

        // Print diff if available
        if let Some(ref diff) = error.diff {
            for line in diff.lines() {
                let colored = if line.starts_with('+') {
                    format!("\x1b[32m{}\x1b[0m", line)
                } else if line.starts_with('-') {
                    format!("\x1b[31m{}\x1b[0m", line)
                } else {
                    line.to_string()
                };
                println!("    {}", colored);
            }
            println!();
        }

        // Print stack trace location
        if let Some(ref location) = error.location {
            println!(
                "      at {}:{}",
                location.file,
                location.line
            );
            println!();
        }
    }
}

fn print_summary(response: &RunResponse) {
    let suites_status = if response.num_failed_suites > 0 {
        format!(
            "\x1b[31m{} failed\x1b[0m, {} passed",
            response.num_failed_suites, response.num_passed_suites
        )
    } else {
        format!("\x1b[32m{} passed\x1b[0m", response.num_passed_suites)
    };

    let tests_status = if response.num_failed_tests > 0 {
        format!(
            "\x1b[31m{} failed\x1b[0m, {} passed",
            response.num_failed_tests, response.num_passed_tests
        )
    } else {
        format!("\x1b[32m{} passed\x1b[0m", response.num_passed_tests)
    };

    let total_suites = response.num_passed_suites + response.num_failed_suites;
    let total_tests = response.num_passed_tests
        + response.num_failed_tests
        + response.num_skipped_tests
        + response.num_todo_tests;

    println!(
        "Test Suites: {}, {} total",
        suites_status, total_suites
    );
    println!(
        "Tests:       {}, {} total",
        tests_status, total_tests
    );

    if response.num_skipped_tests > 0 {
        println!("Skipped:     {}", response.num_skipped_tests);
    }
    if response.num_todo_tests > 0 {
        println!("Todo:        {}", response.num_todo_tests);
    }

    // Print snapshot summary if present
    if let Some(ref snap) = response.snapshot_summary {
        let mut parts = vec![];
        if snap.matched > 0 {
            parts.push(format!("{} passed", snap.matched));
        }
        if snap.added > 0 {
            parts.push(format!("{} added", snap.added));
        }
        if snap.updated > 0 {
            parts.push(format!("{} updated", snap.updated));
        }
        if snap.unmatched > 0 {
            parts.push(format!("\x1b[31m{} failed\x1b[0m", snap.unmatched));
        }
        if !parts.is_empty() {
            println!("Snapshots:   {}", parts.join(", "));
        }
    }

    // Print timing
    let duration_secs = response.duration_ms as f64 / 1000.0;
    println!("Time:        {:.3} s", duration_secs);
}
