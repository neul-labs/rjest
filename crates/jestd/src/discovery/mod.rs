use anyhow::Result;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};
use tracing::{debug, info};
use walkdir::WalkDir;

use crate::config::JestConfig;

/// How long to cache discovery results before re-scanning
const DISCOVERY_CACHE_TTL: Duration = Duration::from_secs(5);

/// Global discovery cache
lazy_static::lazy_static! {
    static ref DISCOVERY_CACHE: Arc<RwLock<DiscoveryCache>> = Arc::new(RwLock::new(DiscoveryCache::new()));
}

/// Cached discovery result
struct CachedDiscovery {
    test_files: Vec<PathBuf>,
    timestamp: Instant,
    config_hash: u64,
}

/// Cache for test file discovery
struct DiscoveryCache {
    entries: HashMap<PathBuf, CachedDiscovery>,
}

impl DiscoveryCache {
    fn new() -> Self {
        Self {
            entries: HashMap::new(),
        }
    }

    fn get(&self, root: &Path, config_hash: u64) -> Option<Vec<PathBuf>> {
        if let Some(entry) = self.entries.get(root) {
            if entry.config_hash == config_hash && entry.timestamp.elapsed() < DISCOVERY_CACHE_TTL {
                debug!("Discovery cache hit for {}", root.display());
                return Some(entry.test_files.clone());
            }
        }
        None
    }

    fn set(&mut self, root: PathBuf, test_files: Vec<PathBuf>, config_hash: u64) {
        self.entries.insert(root, CachedDiscovery {
            test_files,
            timestamp: Instant::now(),
            config_hash,
        });
    }

    fn invalidate(&mut self, root: &Path) {
        self.entries.remove(root);
    }
}

/// Invalidate discovery cache for a root directory
pub fn invalidate_discovery_cache(root: &Path) {
    if let Ok(mut cache) = DISCOVERY_CACHE.write() {
        cache.invalidate(root);
    }
}

/// Discover test files in a project based on Jest configuration
pub struct TestDiscovery {
    config: JestConfig,
    config_hash: u64,
}

impl TestDiscovery {
    pub fn new(config: JestConfig) -> Self {
        // Compute a hash of the config for cache invalidation
        let config_hash = compute_config_hash(&config);
        Self { config, config_hash }
    }

    /// Find all test files matching the configuration
    pub fn find_tests(&self) -> Result<Vec<PathBuf>> {
        // Check cache first
        if let Ok(cache) = DISCOVERY_CACHE.read() {
            if let Some(cached) = cache.get(&self.config.root_dir, self.config_hash) {
                info!("Using cached discovery ({} files)", cached.len());
                return Ok(cached);
            }
        }

        // Perform actual discovery
        let test_files = self.discover_tests()?;

        // Cache the results
        if let Ok(mut cache) = DISCOVERY_CACHE.write() {
            cache.set(self.config.root_dir.clone(), test_files.clone(), self.config_hash);
        }

        Ok(test_files)
    }

    /// Actually discover test files (uncached)
    fn discover_tests(&self) -> Result<Vec<PathBuf>> {
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

/// Compute a hash of the config for cache invalidation
fn compute_config_hash(config: &JestConfig) -> u64 {
    use std::hash::{Hash, Hasher};
    use std::collections::hash_map::DefaultHasher;

    let mut hasher = DefaultHasher::new();

    // Hash key config fields that affect discovery
    for root in &config.roots {
        root.to_string_lossy().hash(&mut hasher);
    }
    for ext in &config.module_file_extensions {
        ext.hash(&mut hasher);
    }
    for pattern in &config.test_match {
        pattern.hash(&mut hasher);
    }
    for regex in &config.test_regex {
        regex.hash(&mut hasher);
    }

    hasher.finish()
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
