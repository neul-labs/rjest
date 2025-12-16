use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use tracing::{debug, info};

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
}

fn default_timeout() -> u32 {
    5000
}

impl JestConfig {
    /// Load Jest configuration for a project by invoking Node
    pub fn load(project_root: &Path) -> Result<Self> {
        info!("Loading Jest config for {}", project_root.display());

        // Find the config loader script
        let loader_script = find_config_loader()?;

        // Run the Node script
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

        let config: JestConfig =
            serde_json::from_str(&stdout).context("Failed to parse config JSON")?;

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

        // Check ignore patterns first
        for pattern in &self.test_path_ignore_patterns {
            if path_str.contains(pattern.trim_start_matches('/').trim_end_matches('/')) {
                return false;
            }
        }

        // Check testMatch patterns
        for pattern in &self.test_match {
            if glob_match(pattern, &path_str) {
                return true;
            }
        }

        // Check testRegex patterns
        for pattern in &self.test_regex {
            if let Ok(re) = regex::Regex::new(pattern) {
                if re.is_match(&path_str) {
                    return true;
                }
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

/// Glob matching for Jest test patterns
fn glob_match(pattern: &str, path: &str) -> bool {
    // Convert Jest/minimatch glob pattern to regex
    // Handle common patterns:
    // - ** = any path segments
    // - * = any chars except /
    // - ? = single char (or optional group in extended glob)
    // - [abc] = char class
    // - ?(x) = zero or one of x
    // - +(x) = one or more of x
    // - *(x) = zero or more of x
    // - @(x|y) = one of x or y

    let mut regex_pattern = String::new();
    let chars: Vec<char> = pattern.chars().collect();
    let mut i = 0;

    while i < chars.len() {
        match chars[i] {
            '*' if i + 1 < chars.len() && chars[i + 1] == '*' => {
                // ** = any path (including /)
                regex_pattern.push_str(".*");
                i += 2;
                // Skip trailing /
                if i < chars.len() && chars[i] == '/' {
                    i += 1;
                }
            }
            '*' => {
                // * = any chars except /
                regex_pattern.push_str("[^/]*");
                i += 1;
            }
            '?' if i + 1 < chars.len() && chars[i + 1] == '(' => {
                // ?(x) = zero or one of x
                let (group, end) = extract_group(&chars, i + 1);
                regex_pattern.push_str(&format!("({})?", group));
                i = end + 1;
            }
            '+' if i + 1 < chars.len() && chars[i + 1] == '(' => {
                // +(x) = one or more of x
                let (group, end) = extract_group(&chars, i + 1);
                regex_pattern.push_str(&format!("({})+", group));
                i = end + 1;
            }
            '@' if i + 1 < chars.len() && chars[i + 1] == '(' => {
                // @(x|y) = one of
                let (group, end) = extract_group(&chars, i + 1);
                regex_pattern.push_str(&format!("({})", group));
                i = end + 1;
            }
            '.' => {
                regex_pattern.push_str("\\.");
                i += 1;
            }
            '[' => {
                // Character class - pass through
                let start = i;
                i += 1;
                while i < chars.len() && chars[i] != ']' {
                    i += 1;
                }
                let class: String = chars[start..=i.min(chars.len() - 1)].iter().collect();
                regex_pattern.push_str(&class);
                i += 1;
            }
            '(' | ')' | '{' | '}' | '^' | '$' | '|' | '\\' => {
                regex_pattern.push('\\');
                regex_pattern.push(chars[i]);
                i += 1;
            }
            c => {
                regex_pattern.push(c);
                i += 1;
            }
        }
    }

    match regex::Regex::new(&format!("{}$", regex_pattern)) {
        Ok(re) => re.is_match(path),
        Err(_) => {
            // Fallback: simple substring match
            path.contains(".test.") || path.contains(".spec.") || path.contains("__tests__")
        }
    }
}

fn extract_group(chars: &[char], start: usize) -> (String, usize) {
    // start points to '('
    let mut depth = 0;
    let mut end = start;
    for (i, &c) in chars[start..].iter().enumerate() {
        match c {
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 {
                    end = start + i;
                    break;
                }
            }
            _ => {}
        }
    }
    let inner: String = chars[start + 1..end].iter().collect();
    // Convert | to regex alternation
    let converted = inner.replace('.', "\\.").replace('|', "|");
    (converted, end)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_glob_match() {
        // Simple patterns
        assert!(glob_match("**/*.test.ts", "src/utils.test.ts"));
        assert!(glob_match("**/*.test.ts", "src/foo/bar.test.ts"));
        assert!(glob_match("**/__tests__/**/*.ts", "src/__tests__/foo.ts"));
        assert!(!glob_match("**/*.test.ts", "src/utils.ts"));

        // Jest default patterns
        let jest_pattern = "**/?(*.)+(spec|test).[jt]s?(x)";
        assert!(glob_match(jest_pattern, "src/utils.test.js"));
        assert!(glob_match(jest_pattern, "src/utils.test.ts"));
        assert!(glob_match(jest_pattern, "src/utils.spec.js"));
        assert!(glob_match(jest_pattern, "src/utils.test.tsx"));
        assert!(glob_match(jest_pattern, "foo.test.js"));
    }
}
