use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use sled::Db;
use std::path::{Path, PathBuf};
use std::process::Command;
use tracing::{debug, info, warn};

/// Cached transform result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransformResult {
    /// Compiled JavaScript code
    pub code: String,
    /// Source map (JSON string)
    pub source_map: Option<String>,
    /// Original file path
    pub original_path: PathBuf,
    /// Content hash of source
    pub source_hash: String,
}

/// Transform cache backed by sled
pub struct TransformCache {
    db: Db,
    cache_dir: PathBuf,
}

impl TransformCache {
    /// Open or create a transform cache
    pub fn open(cache_dir: &Path) -> Result<Self> {
        let db_path = cache_dir.join("transforms.sled");
        std::fs::create_dir_all(&cache_dir)?;

        let db = sled::open(&db_path).context("Failed to open transform cache")?;

        info!("Opened transform cache at {}", db_path.display());

        Ok(Self {
            db,
            cache_dir: cache_dir.to_path_buf(),
        })
    }

    /// Get cached transform or None if not found/stale
    pub fn get(&self, path: &Path, source_hash: &str) -> Option<TransformResult> {
        let key = cache_key(path);

        match self.db.get(&key) {
            Ok(Some(data)) => {
                match serde_json::from_slice::<TransformResult>(&data) {
                    Ok(result) if result.source_hash == source_hash => {
                        debug!("Cache hit for {}", path.display());
                        crate::metrics::record_cache_hit();
                        Some(result)
                    }
                    Ok(_) => {
                        debug!("Cache stale for {}", path.display());
                        crate::metrics::record_cache_miss();
                        None
                    }
                    Err(e) => {
                        warn!("Cache corrupted for {}: {}", path.display(), e);
                        crate::metrics::record_cache_miss();
                        None
                    }
                }
            }
            Ok(None) => {
                debug!("Cache miss for {}", path.display());
                crate::metrics::record_cache_miss();
                None
            }
            Err(e) => {
                warn!("Cache read error for {}: {}", path.display(), e);
                None
            }
        }
    }

    /// Store transform result in cache
    pub fn set(&self, path: &Path, result: &TransformResult) -> Result<()> {
        let key = cache_key(path);
        let value = serde_json::to_vec(result)?;
        self.db.insert(key, value)?;
        self.db.flush()?;
        debug!("Cached transform for {}", path.display());
        Ok(())
    }

    /// Get cache statistics
    pub fn stats(&self) -> CacheStats {
        CacheStats {
            entry_count: self.db.len() as u64,
            size_bytes: self.db.size_on_disk().unwrap_or(0),
        }
    }

    /// Clear all cached transforms
    pub fn clear(&self) -> Result<()> {
        self.db.clear()?;
        self.db.flush()?;
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct CacheStats {
    pub entry_count: u64,
    pub size_bytes: u64,
}

fn cache_key(path: &Path) -> Vec<u8> {
    path.to_string_lossy().as_bytes().to_vec()
}

/// Transform pipeline for compiling TypeScript/JSX
pub struct Transformer {
    cache: TransformCache,
}

impl Transformer {
    pub fn new(cache_dir: &Path) -> Result<Self> {
        let cache = TransformCache::open(cache_dir)?;
        Ok(Self { cache })
    }

    /// Transform a source file, using cache if available
    pub fn transform(&self, path: &Path) -> Result<TransformResult> {
        // Read source file
        let source = std::fs::read_to_string(path)
            .with_context(|| format!("Failed to read {}", path.display()))?;

        // Compute content hash
        let source_hash = blake3::hash(source.as_bytes()).to_hex().to_string();

        // Check cache
        if let Some(cached) = self.cache.get(path, &source_hash) {
            return Ok(cached);
        }

        // Transform with SWC
        let result = transform_with_swc(path, &source, &source_hash)?;

        // Cache result
        self.cache.set(path, &result)?;

        Ok(result)
    }

    /// Transform multiple files, returning results in same order
    pub fn transform_many(&self, paths: &[PathBuf]) -> Vec<Result<TransformResult>> {
        paths.iter().map(|p| self.transform(p)).collect()
    }

    /// Get cache statistics
    pub fn cache_stats(&self) -> CacheStats {
        self.cache.stats()
    }
}

/// Transform a file using SWC CLI
/// TODO: Replace with native SWC integration for better performance
fn transform_with_swc(path: &Path, source: &str, source_hash: &str) -> Result<TransformResult> {
    let extension = path.extension().and_then(|e| e.to_str()).unwrap_or("");

    // Determine if we need to transform
    let needs_transform = matches!(extension, "ts" | "tsx" | "jsx" | "mts" | "cts");

    if !needs_transform {
        // Plain JS, just return as-is
        return Ok(TransformResult {
            code: source.to_string(),
            source_map: None,
            original_path: path.to_path_buf(),
            source_hash: source_hash.to_string(),
        });
    }

    // Try using npx swc
    let output = Command::new("npx")
        .args([
            "swc",
            path.to_str().unwrap(),
            "--out-file",
            "/dev/stdout",
            "--source-maps",
            "false",
            "--config-file",
            "false",
            "-C",
            "jsc.parser.syntax=typescript",
            "-C",
            "jsc.parser.tsx=true",
            "-C",
            "jsc.target=es2020",
            "-C",
            "module.type=commonjs",
        ])
        .output();

    match output {
        Ok(output) if output.status.success() => {
            let code = String::from_utf8_lossy(&output.stdout).to_string();
            Ok(TransformResult {
                code,
                source_map: None,
                original_path: path.to_path_buf(),
                source_hash: source_hash.to_string(),
            })
        }
        Ok(output) => {
            let stderr = String::from_utf8_lossy(&output.stderr);
            // Fall back to esbuild or basic transform
            warn!("SWC failed for {}: {}", path.display(), stderr);
            transform_fallback(path, source, source_hash)
        }
        Err(_) => {
            // SWC not available, try fallback
            transform_fallback(path, source, source_hash)
        }
    }
}

/// Fallback transform using esbuild or basic stripping
fn transform_fallback(path: &Path, source: &str, source_hash: &str) -> Result<TransformResult> {
    // Try esbuild
    let output = Command::new("npx")
        .args([
            "esbuild",
            "--loader=tsx",
            "--format=cjs",
            "--target=es2020",
        ])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn();

    if let Ok(mut child) = output {
        use std::io::Write;
        if let Some(stdin) = child.stdin.as_mut() {
            let _ = stdin.write_all(source.as_bytes());
        }

        if let Ok(output) = child.wait_with_output() {
            if output.status.success() {
                let code = String::from_utf8_lossy(&output.stdout).to_string();
                return Ok(TransformResult {
                    code,
                    source_map: None,
                    original_path: path.to_path_buf(),
                    source_hash: source_hash.to_string(),
                });
            }
        }
    }

    // Last resort: strip type annotations with a basic regex
    // This is very limited but allows tests to at least attempt to run
    warn!(
        "No transformer available for {}, using basic type stripping",
        path.display()
    );
    let code = basic_type_strip(source);

    Ok(TransformResult {
        code,
        source_map: None,
        original_path: path.to_path_buf(),
        source_hash: source_hash.to_string(),
    })
}

/// Very basic type annotation stripping (last resort)
fn basic_type_strip(source: &str) -> String {
    // This is intentionally very simple and won't handle all cases
    // It's only meant as a fallback when no proper transformer is available
    let mut result = source.to_string();

    // Remove type imports
    if let Ok(re) = regex::Regex::new(r"import\s+type\s+[^;]+;") {
        result = re.replace_all(&result, "").to_string();
    }

    // Remove simple type annotations (: Type) - very basic
    // This handles : number, : string, : void, : boolean, etc.
    if let Ok(re) = regex::Regex::new(r":\s*(number|string|boolean|void|any|null|undefined|never)\b") {
        result = re.replace_all(&result, "").to_string();
    }

    // Remove type assertions (as Type)
    if let Ok(re) = regex::Regex::new(r"\s+as\s+\w+") {
        result = re.replace_all(&result, "").to_string();
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_type_strip() {
        let input = r#"
const x: number = 1;
const y: string = "hello";
function foo(a: number, b: string): void {}
"#;
        let output = basic_type_strip(input);
        assert!(!output.contains(": number"));
        assert!(!output.contains(": string"));
        assert!(!output.contains(": void"));
    }
}
