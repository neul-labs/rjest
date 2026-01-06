use anyhow::{Context, Result};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use tracing::{debug, info};

/// Cached compiled patterns for JestConfig
#[derive(Debug, Clone)]
pub struct JestConfigPatterns {
    pub test_regex_compiled: Vec<Regex>,
    pub test_path_ignore_compiled: Vec<Regex>,
    pub coverage_path_ignore_compiled: Vec<Regex>,
    pub transform_ignore_compiled: Vec<Regex>,
}

/// Normalized Jest configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JestConfig {
    pub root_dir: PathBuf,
    pub roots: Vec<PathBuf>,
    pub test_match: Vec<String>,
    #[serde(default)]
    pub test_regex: Vec<String>,
    pub test_path_ignore_patterns: Vec<String>,
    pub module_file_extensions: Vec<String>,
    #[serde(default)]
    pub module_name_mapper: HashMap<String, String>,
    pub module_directories: Vec<String>,
    #[serde(default)]
    pub module_paths: Vec<String>,
    #[serde(default)]
    pub transform: HashMap<String, String>,
    pub transform_ignore_patterns: Vec<String>,
    #[serde(default)]
    pub setup_files: Vec<PathBuf>,
    #[serde(default)]
    pub setup_files_after_env: Vec<PathBuf>,
    pub test_environment: String,
    #[serde(default)]
    pub test_environment_options: HashMap<String, serde_json::Value>,
    #[serde(default)]
    pub globals: HashMap<String, serde_json::Value>,
    #[serde(default)]
    pub collect_coverage: bool,
    pub collect_coverage_from: Option<Vec<String>>,
    pub coverage_directory: PathBuf,
    pub coverage_path_ignore_patterns: Vec<String>,
    pub coverage_reporters: Vec<String>,
    #[serde(default)]
    pub snapshot_serializers: Vec<String>,
    #[serde(default = "default_timeout")]
    pub test_timeout: u32,
    #[serde(default)]
    pub verbose: bool,
    #[serde(default)]
    pub bail: u32,
    pub max_workers: serde_json::Value,
    pub projects: Option<Vec<serde_json::Value>>,
    pub display_name: Option<String>,
    #[serde(default)]
    pub clear_mocks: bool,
    #[serde(default)]
    pub reset_mocks: bool,
    #[serde(default)]
    pub restore_mocks: bool,
    /// Cached compiled patterns (computed lazily)
    #[serde(skip)]
    pub patterns: Option<JestConfigPatterns>,
}

fn default_timeout() -> u32 {
    5000
}

impl JestConfig {
    /// Compile and cache regex patterns for efficient matching
    pub fn compile_patterns(&mut self) {
        if self.patterns.is_some() {
            return; // Already compiled
        }

        let test_regex_compiled: Vec<Regex> = self.test_regex
            .iter()
            .filter_map(|p| Regex::new(p).ok())
            .collect();

        let test_path_ignore_compiled: Vec<Regex> = self.test_path_ignore_patterns
            .iter()
            .filter_map(|p| Regex::new(p).ok())
            .collect();

        let coverage_path_ignore_compiled: Vec<Regex> = self.coverage_path_ignore_patterns
            .iter()
            .filter_map(|p| Regex::new(p).ok())
            .collect();

        let transform_ignore_compiled: Vec<Regex> = self.transform_ignore_patterns
            .iter()
            .filter_map(|p| Regex::new(p).ok())
            .collect();

        self.patterns = Some(JestConfigPatterns {
            test_regex_compiled,
            test_path_ignore_compiled,
            coverage_path_ignore_compiled,
            transform_ignore_compiled,
        });
    }

    /// Get compiled patterns, compiling if necessary
    fn get_patterns(&mut self) -> &JestConfigPatterns {
        if self.patterns.is_none() {
            self.compile_patterns();
        }
        self.patterns.as_ref().unwrap()
    }
    /// Load Jest configuration for a project by invoking Node
    pub fn load(project_root: &Path) -> Result<Self> {
        info!("Loading Jest config for {}", project_root.display());

        // Find the config loader script
        let loader_script = find_config_loader()?;

        // Run the Node script (blocking)
        let output = Command::new("node")
            .arg(&loader_script)
            .arg(project_root)
            .current_dir(project_root)
            .output()
            .context("Failed to execute config loader")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("Config loader failed: {}", stderr);
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        debug!("Config loader output: {}", stdout);

        // Check for error response
        if let Ok(error) = serde_json::from_str::<ConfigError>(&stdout) {
            if error.error {
                anyhow::bail!("Failed to load config: {}", error.message);
            }
        }

        let mut config: JestConfig =
            serde_json::from_str(&stdout).context("Failed to parse config JSON")?;

        // Compile regex patterns upfront for efficient matching
        config.compile_patterns();

        info!("Loaded config with {} roots", config.roots.len());
        Ok(config)
    }

    /// Async version of load using tokio::process
    pub async fn load_async(project_root: &Path) -> Result<Self> {
        use tokio::process::Command;

        info!("Loading Jest config asynchronously for {}", project_root.display());

        // Find the config loader script
        let loader_script = find_config_loader()?;

        // Run the Node script asynchronously
        let output = Command::new("node")
            .arg(&loader_script)
            .arg(project_root)
            .current_dir(project_root)
            .output()
            .await
            .context("Failed to execute config loader")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("Config loader failed: {}", stderr);
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        debug!("Config loader output: {}", stdout);

        // Check for error response
        if let Ok(error) = serde_json::from_str::<ConfigError>(&stdout) {
            if error.error {
                anyhow::bail!("Failed to load config: {}", error.message);
            }
        }

        let mut config: JestConfig =
            serde_json::from_str(&stdout).context("Failed to parse config JSON")?;

        // Compile regex patterns upfront for efficient matching
        config.compile_patterns();

        info!("Loaded config with {} roots", config.roots.len());
        Ok(config)
    }

    /// Get the effective max workers count
    pub fn max_workers_count(&self) -> usize {
        match &self.max_workers {
            serde_json::Value::Number(n) => n.as_u64().unwrap_or(1) as usize,
            serde_json::Value::String(s) => {
                if s.ends_with('%') {
                    let percent: f64 = s.trim_end_matches('%').parse().unwrap_or(50.0);
                    let cpus = num_cpus();
                    ((cpus as f64 * percent / 100.0).ceil() as usize).max(1)
                } else {
                    s.parse().unwrap_or(1)
                }
            }
            _ => num_cpus().max(1),
        }
    }

    /// Check if a file path matches the test patterns
    pub fn is_test_file(&self, path: &Path) -> bool {
        let path_str = path.to_string_lossy();

        // Get cached patterns (always Some after load())
        let patterns = match &self.patterns {
            Some(p) => p,
            None => return false, // Should not happen after load()
        };

        // Check ignore patterns first
        for re in &patterns.test_path_ignore_compiled {
            if re.is_match(&path_str) {
                return false;
            }
        }

        // Check testMatch patterns using shared glob_match
        for pattern in &self.test_match {
            if crate::rjest_util::glob_match(pattern, &path_str) {
                return true;
            }
        }

        // Check testRegex patterns using cached compiled regex
        for re in &patterns.test_regex_compiled {
            if re.is_match(&path_str) {
                return true;
            }
        }

        false
    }
}

#[derive(Debug, Deserialize)]
struct ConfigError {
    error: bool,
    message: String,
}

/// Find the config loader script bundled with jestd
fn find_config_loader() -> Result<PathBuf> {
    // First, check relative to the executable
    if let Ok(exe) = std::env::current_exe() {
        if let Some(exe_dir) = exe.parent() {
            // Check various possible locations
            let candidates = [
                exe_dir.join("../lib/rjest-runtime/src/load-config.js"),
                exe_dir.join("../../crates/rjest-runtime/src/load-config.js"),
                exe_dir.join("load-config.js"),
            ];

            for candidate in candidates {
                if candidate.exists() {
                    return Ok(candidate.canonicalize()?);
                }
            }
        }
    }

    // Fallback: check relative to CARGO_MANIFEST_DIR (for development)
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let dev_path = PathBuf::from(manifest_dir)
        .join("../rjest-runtime/src/load-config.js");
    if dev_path.exists() {
        return Ok(dev_path.canonicalize()?);
    }

    anyhow::bail!("Could not find config loader script")
}

fn num_cpus() -> usize {
    std::thread::available_parallelism()
        .map(|p| p.get())
        .unwrap_or(4)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_glob_match() {
        // Simple patterns using shared rjest_util::glob_match
        assert!(crate::rjest_util::glob_match("**/*.test.ts", "src/utils.test.ts"));
        assert!(crate::rjest_util::glob_match("**/*.test.ts", "src/foo/bar.test.ts"));
        assert!(crate::rjest_util::glob_match("**/__tests__/**/*.ts", "src/__tests__/foo.ts"));
        assert!(!crate::rjest_util::glob_match("**/*.test.ts", "src/utils.ts"));

        // Jest default patterns
        let jest_pattern = "**/?(*.)+(spec|test).[jt]s?(x)";
        assert!(crate::rjest_util::glob_match(jest_pattern, "src/utils.test.js"));
        assert!(crate::rjest_util::glob_match(jest_pattern, "src/utils.test.ts"));
        assert!(crate::rjest_util::glob_match(jest_pattern, "src/utils.spec.js"));
        assert!(crate::rjest_util::glob_match(jest_pattern, "src/utils.test.tsx"));
        assert!(crate::rjest_util::glob_match(jest_pattern, "foo.test.js"));
    }
}
