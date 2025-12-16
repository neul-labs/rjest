use anyhow::Result;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use tracing::{debug, info};
use walkdir::WalkDir;

use crate::config::JestConfig;

/// Discover test files in a project based on Jest configuration
pub struct TestDiscovery {
    config: JestConfig,
}

impl TestDiscovery {
    pub fn new(config: JestConfig) -> Self {
        Self { config }
    }

    /// Find all test files matching the configuration
    pub fn find_tests(&self) -> Result<Vec<PathBuf>> {
        let mut test_files = HashSet::new();

        for root in &self.config.roots {
            let root_path = if root.is_absolute() {
                root.clone()
            } else {
                self.config.root_dir.join(root)
            };

            debug!("Searching for tests in {}", root_path.display());

            if !root_path.exists() {
                debug!("Root path does not exist: {}", root_path.display());
                continue;
            }

            for entry in WalkDir::new(&root_path)
                .follow_links(true)
                .into_iter()
                .filter_entry(|e| !is_hidden(e) && !is_node_modules(e))
            {
                let entry = match entry {
                    Ok(e) => e,
                    Err(_) => continue,
                };

                if !entry.file_type().is_file() {
                    continue;
                }

                let path = entry.path();

                // Check if file has a valid extension
                if !has_valid_extension(path, &self.config.module_file_extensions) {
                    continue;
                }

                // Check if file matches test patterns
                if self.config.is_test_file(path) {
                    debug!("Found test file: {}", path.display());
                    test_files.insert(path.to_path_buf());
                }
            }
        }

        let mut result: Vec<PathBuf> = test_files.into_iter().collect();
        result.sort();

        info!("Discovered {} test files", result.len());
        Ok(result)
    }

    /// Find tests matching specific patterns
    pub fn find_tests_matching(&self, patterns: &[String]) -> Result<Vec<PathBuf>> {
        if patterns.is_empty() {
            return self.find_tests();
        }

        let all_tests = self.find_tests()?;

        let filtered: Vec<PathBuf> = all_tests
            .into_iter()
            .filter(|path| {
                let path_str = path.to_string_lossy();
                patterns.iter().any(|pattern| {
                    // Simple substring match or glob
                    path_str.contains(pattern) || glob_match(pattern, &path_str)
                })
            })
            .collect();

        info!(
            "Filtered to {} test files matching patterns",
            filtered.len()
        );
        Ok(filtered)
    }

    /// Find tests related to specific source files
    pub fn find_related_tests(&self, source_files: &[PathBuf]) -> Result<Vec<PathBuf>> {
        // For now, do a simple heuristic: find test files with similar names
        // TODO: Use dependency graph for accurate related test detection

        let all_tests = self.find_tests()?;

        let related: Vec<PathBuf> = all_tests
            .into_iter()
            .filter(|test_path| {
                let test_stem = test_path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("");

                // Remove .test or .spec suffix to get base name
                let base_name = test_stem
                    .trim_end_matches(".test")
                    .trim_end_matches(".spec");

                source_files.iter().any(|source| {
                    let source_stem = source
                        .file_stem()
                        .and_then(|s| s.to_str())
                        .unwrap_or("");

                    source_stem == base_name
                        || test_path
                            .parent()
                            .map(|p| source.starts_with(p) || p.starts_with(source.parent().unwrap_or(Path::new(""))))
                            .unwrap_or(false)
                })
            })
            .collect();

        info!("Found {} related test files", related.len());
        Ok(related)
    }
}

fn is_hidden(entry: &walkdir::DirEntry) -> bool {
    entry
        .file_name()
        .to_str()
        .map(|s| s.starts_with('.'))
        .unwrap_or(false)
}

fn is_node_modules(entry: &walkdir::DirEntry) -> bool {
    entry
        .file_name()
        .to_str()
        .map(|s| s == "node_modules")
        .unwrap_or(false)
}

fn has_valid_extension(path: &Path, extensions: &[String]) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| extensions.iter().any(|e| e == ext))
        .unwrap_or(false)
}

fn glob_match(pattern: &str, path: &str) -> bool {
    // Simple glob matching
    let regex_pattern = pattern
        .replace(".", "\\.")
        .replace("**", "{{GLOBSTAR}}")
        .replace("*", "[^/]*")
        .replace("{{GLOBSTAR}}", ".*")
        .replace("?", ".");

    regex::Regex::new(&format!("{}$", regex_pattern))
        .map(|re| re.is_match(path))
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_hidden() {
        // Would need to construct DirEntry for proper testing
    }

    #[test]
    fn test_has_valid_extension() {
        let exts = vec!["ts".to_string(), "tsx".to_string(), "js".to_string()];
        assert!(has_valid_extension(Path::new("foo.ts"), &exts));
        assert!(has_valid_extension(Path::new("foo.tsx"), &exts));
        assert!(!has_valid_extension(Path::new("foo.rs"), &exts));
    }
}
