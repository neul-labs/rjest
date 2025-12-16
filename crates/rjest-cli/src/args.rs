use anyhow::Result;
use clap::Parser;
use rjest_protocol::{RunFlags, RunRequest};

#[derive(Parser, Debug)]
#[command(
    name = "jest",
    about = "Fast Jest-compatible test runner powered by rjest",
    version,
    after_help = "For more information, see https://github.com/user/rjest"
)]
pub struct Args {
    /// Test file patterns to run
    #[arg(num_args = 0..)]
    pub patterns: Vec<String>,

    /// Run all tests serially in the current process
    #[arg(short = 'i', long = "runInBand")]
    pub run_in_band: bool,

    /// Watch files for changes and rerun related tests
    #[arg(long)]
    pub watch: bool,

    /// Watch files for changes and rerun all tests
    #[arg(long = "watchAll")]
    pub watch_all: bool,

    /// Exit immediately after first test failure
    #[arg(short, long)]
    pub bail: bool,

    /// Print test results in JSON format
    #[arg(long)]
    pub json: bool,

    /// Machine-readable output optimized for AI agents
    #[arg(long)]
    pub machine: bool,

    /// Display individual test results
    #[arg(long)]
    pub verbose: bool,

    /// Specifies the maximum number of workers
    #[arg(short = 'w', long = "maxWorkers")]
    pub max_workers: Option<u32>,

    /// Path to Jest configuration file
    #[arg(short = 'c', long)]
    pub config: Option<String>,

    /// Only run tests related to changed files
    #[arg(short = 'o', long = "onlyChanged")]
    pub only_changed: bool,

    /// Run tests related to specific files
    #[arg(long = "findRelatedTests", num_args = 1..)]
    pub find_related_tests: Vec<String>,

    /// Update snapshots
    #[arg(short = 'u', long = "updateSnapshot")]
    pub update_snapshots: bool,

    /// Collect test coverage
    #[arg(long)]
    pub coverage: bool,

    /// Run only tests matching this pattern
    #[arg(short = 't', long = "testNamePattern")]
    pub test_name_pattern: Option<String>,

    /// Force fallback to upstream Jest
    #[arg(long = "fallback-to-jest")]
    pub fallback_to_jest: bool,

    /// Show daemon status
    #[arg(long = "daemon-status")]
    pub daemon_status: bool,

    /// Stop the daemon
    #[arg(long = "daemon-stop")]
    pub daemon_stop: bool,

    /// Restart the daemon
    #[arg(long = "daemon-restart")]
    pub daemon_restart: bool,

    /// Health check - show detailed daemon health information
    #[arg(long = "daemon-health")]
    pub daemon_health: bool,
}

impl Args {
    /// Convert CLI args to a RunRequest for the daemon
    pub fn to_run_request(&self) -> Result<RunRequest> {
        let project_root = std::env::current_dir()?
            .to_string_lossy()
            .to_string();

        Ok(RunRequest {
            project_root,
            patterns: self.patterns.clone(),
            flags: self.to_run_flags(),
        })
    }

    /// Extract just the RunFlags from args
    pub fn to_run_flags(&self) -> RunFlags {
        RunFlags {
            run_in_band: self.run_in_band,
            watch: self.watch || self.watch_all,
            bail: self.bail,
            json_output: self.json,
            machine_output: self.machine,
            max_workers: self.max_workers,
            config_path: self.config.clone(),
            only_changed: self.only_changed,
            find_related_tests: self.find_related_tests.clone(),
            update_snapshots: self.update_snapshots,
            coverage: self.coverage,
            test_name_pattern: self.test_name_pattern.clone(),
            verbose: self.verbose,
        }
    }
}
