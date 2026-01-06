use anyhow::{Context, Result};
use lru::LruCache;
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use sled::Db;
use std::num::NonZeroUsize;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use tracing::{info, warn};
use swc_common::comments::SingleThreadedComments;
use swc_common::sync::Lrc;
use swc_common::{FileName, Globals, Mark, SourceMap, GLOBALS};
use swc_ecma_ast::{EsVersion, Pass, Program};
use swc_ecma_codegen::text_writer::JsWriter;
use swc_ecma_codegen::Emitter;
use swc_ecma_parser::{parse_file_as_module, EsSyntax, Syntax, TsSyntax};
use swc_ecma_transforms_base::fixer::fixer;
use swc_ecma_transforms_base::hygiene::hygiene;
use swc_ecma_transforms_base::resolver;
use swc_ecma_transforms_module::common_js::{self, FeatureFlag};
use swc_ecma_transforms_module::path::Resolver;
use swc_ecma_transforms_typescript::strip;
use swc_ecma_visit::visit_mut_pass;

/// Size of the in-memory LRU cache for transforms
const LRU_CACHE_SIZE: usize = 1000;

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

/// Transform cache backed by sled with LRU in-memory layer
pub struct TransformCache {
    db: Db,
    /// In-memory LRU cache for hot files
    lru: Mutex<LruCache<String, TransformResult>>,
}

impl TransformCache {
    /// Open or create a transform cache
    pub fn open(cache_dir: &Path) -> Result<Self> {
        let db_path = cache_dir.join("transforms.sled");
        std::fs::create_dir_all(cache_dir)?;

        // Configure sled for performance
        let db = sled::Config::new()
            .path(&db_path)
            .cache_capacity(64 * 1024 * 1024) // 64MB cache
            .flush_every_ms(Some(5000)) // Async flush every 5 seconds
            .open()
            .context("Failed to open transform cache")?;

        info!("Opened transform cache at {}", db_path.display());

        let lru = LruCache::new(NonZeroUsize::new(LRU_CACHE_SIZE).unwrap());

        Ok(Self {
            db,
            lru: Mutex::new(lru),
        })
    }

    /// Get cached transform or None if not found/stale
    #[inline]
    pub fn get(&self, path: &Path, source_hash: &str) -> Option<TransformResult> {
        let key = path.to_string_lossy();
        let key_str = key.as_ref();

        // Check LRU cache first (fast path)
        {
            let mut lru = match self.lru.lock() {
                Ok(lru) => lru,
                Err(e) => {
                    warn!("LRU cache lock poisoned, treating as cache miss: {}", e);
                    return None;
                }
            };
            if let Some(result) = lru.get(key_str) {
                if result.source_hash == source_hash {
                    crate::metrics::record_cache_hit();
                    return Some(result.clone());
                }
            }
        }

        // Check sled cache
        let key_bytes = key_str.as_bytes();
        match self.db.get(key_bytes) {
            Ok(Some(data)) => {
                match serde_json::from_slice::<TransformResult>(&data) {
                    Ok(result) if result.source_hash == source_hash => {
                        crate::metrics::record_cache_hit();

                        // Promote to LRU cache
                        let mut lru = match self.lru.lock() {
                            Ok(lru) => lru,
                            Err(e) => {
                                warn!("LRU cache lock poisoned, skipping cache promotion: {}", e);
                                return Some(result);
                            }
                        };
                        lru.put(key_str.to_string(), result.clone());

                        Some(result)
                    }
                    Ok(_) => {
                        crate::metrics::record_cache_miss();
                        None
                    }
                    Err(e) => {
                        warn!("Failed to deserialize cached transform: {}", e);
                        crate::metrics::record_cache_miss();
                        None
                    }
                }
            }
            Ok(None) => {
                crate::metrics::record_cache_miss();
                None
            }
            Err(e) => {
                warn!("Sled cache read error (treating as cache miss): {}", e);
                None
            }
        }
    }

    /// Store transform result in cache (non-blocking)
    #[inline]
    pub fn set(&self, path: &Path, result: &TransformResult) {
        let key = path.to_string_lossy().to_string();

        // Store in LRU cache (fast, synchronous)
        {
            match self.lru.lock() {
                Ok(mut lru) => {
                    lru.put(key.clone(), result.clone());
                }
                Err(e) => {
                    warn!("LRU cache lock poisoned, skipping LRU cache: {}", e);
                    // Continue to try sled cache even if LRU fails
                }
            }
        }

        // Store in sled cache using blocking I/O
        // Note: sled operations are blocking but fast, and we don't await them
        let db = self.db.clone();
        let key_bytes = key.into_bytes();
        let value = serde_json::to_vec(result).ok();

        if let Some(value) = value {
            // Direct blocking insert (sled handles this efficiently)
            if let Err(e) = db.insert(&key_bytes, value) {
                warn!("Failed to persist transform to sled cache: {}", e);
            }
            // sled handles flush asynchronously, no explicit flush needed
        }
    }
}

/// Transform pipeline for compiling TypeScript/JSX
pub struct Transformer {
    cache: TransformCache,
    /// Shared globals for SWC transforms
    globals: Globals,
}

impl Transformer {
    pub fn new(cache_dir: &Path) -> Result<Self> {
        let cache = TransformCache::open(cache_dir)?;
        Ok(Self {
            cache,
            globals: Globals::default(),
        })
    }

    /// Transform a source file, using cache if available
    #[inline]
    pub fn transform(&self, path: &Path) -> Result<TransformResult> {
        // Read source file
        let source = std::fs::read_to_string(path)
            .with_context(|| format!("Failed to read {}", path.display()))?;

        // Compute content hash (blake3 is fast)
        let source_hash = blake3::hash(source.as_bytes()).to_hex().to_string();

        // Check cache first
        if let Some(cached) = self.cache.get(path, &source_hash) {
            return Ok(cached);
        }

        // Transform with native SWC using shared globals
        let result = GLOBALS.set(&self.globals, || {
            transform_with_native_swc(path, &source, &source_hash)
        })?;

        // Cache result (non-blocking)
        self.cache.set(path, &result);

        Ok(result)
    }

    /// Transform multiple files in parallel, returning results in same order
    pub fn transform_many(&self, paths: &[PathBuf]) -> Vec<Result<TransformResult>> {
        paths
            .par_iter()
            .map(|p| self.transform(p))
            .collect()
    }
}

/// Transform a file using native SWC integration
#[inline]
fn transform_with_native_swc(path: &Path, source: &str, source_hash: &str) -> Result<TransformResult> {
    let extension = path.extension().and_then(|e| e.to_str()).unwrap_or("");

    // Fast path: no transform needed for plain JS
    if !matches!(extension, "ts" | "tsx" | "jsx" | "mts" | "cts") {
        return Ok(TransformResult {
            code: source.to_string(),
            source_map: None,
            original_path: path.to_path_buf(),
            source_hash: source_hash.to_string(),
        });
    }

    // Transform TypeScript/JSX
    let code = transform_typescript(path, source, extension)?;

    Ok(TransformResult {
        code,
        source_map: None,
        original_path: path.to_path_buf(),
        source_hash: source_hash.to_string(),
    })
}

/// Transform TypeScript/TSX/JSX to JavaScript using native SWC
fn transform_typescript(path: &Path, source: &str, extension: &str) -> Result<String> {
    let cm: Lrc<SourceMap> = Default::default();
    let comments = SingleThreadedComments::default();

    let fm = cm.new_source_file(
        Lrc::new(FileName::Real(path.to_path_buf())),
        source.to_string(),
    );

    // Determine syntax based on extension
    let syntax = match extension {
        "ts" | "mts" | "cts" => Syntax::Typescript(TsSyntax {
            tsx: false,
            decorators: true,
            dts: false,
            no_early_errors: true,
            ..Default::default()
        }),
        "tsx" => Syntax::Typescript(TsSyntax {
            tsx: true,
            decorators: true,
            dts: false,
            no_early_errors: true,
            ..Default::default()
        }),
        "jsx" => Syntax::Es(EsSyntax {
            jsx: true,
            decorators: true,
            decorators_before_export: true,
            ..Default::default()
        }),
        _ => Syntax::Es(EsSyntax::default()),
    };

    // Parse the file
    let mut errors = vec![];
    let module = parse_file_as_module(&fm, syntax, EsVersion::Es2020, Some(&comments), &mut errors)
        .map_err(|e| anyhow::anyhow!("Parse error: {:?}", e))?;

    if !errors.is_empty() {
        warn!("Parse warnings for {}: {:?}", path.display(), errors);
    }

    // Apply transforms using Program
    let unresolved_mark = Mark::new();
    let top_level_mark = Mark::new();

    // Convert Module to Program for Pass processing
    let mut program = Program::Module(module);

    // Apply resolver
    visit_mut_pass(resolver(unresolved_mark, top_level_mark, true)).process(&mut program);

    // Strip TypeScript types (only for TS files)
    if matches!(extension, "ts" | "tsx" | "mts" | "cts") {
        strip(unresolved_mark, top_level_mark).process(&mut program);
    }

    // Convert ES modules to CommonJS
    let cjs_config = common_js::Config {
        strict_mode: false,
        ..Default::default()
    };
    common_js::common_js(
        Resolver::Default,
        unresolved_mark,
        cjs_config,
        FeatureFlag::default(),
    )
    .process(&mut program);

    // Apply hygiene and fixer
    visit_mut_pass(hygiene()).process(&mut program);
    visit_mut_pass(fixer(Some(&comments))).process(&mut program);

    // Extract module back from program
    let module = match program {
        Program::Module(m) => m,
        _ => unreachable!("Expected module"),
    };

    // Generate output code with pre-allocated buffer
    let mut buf = Vec::with_capacity(source.len() * 2);
    {
        let mut emitter = Emitter {
            cfg: swc_ecma_codegen::Config::default().with_minify(false),
            cm: cm.clone(),
            comments: Some(&comments),
            wr: JsWriter::new(cm.clone(), "\n", &mut buf, None),
        };

        emitter
            .emit_module(&module)
            .context("Failed to emit module")?;
    }

    String::from_utf8(buf).context("Invalid UTF-8 in generated code")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_transform_typescript() {
        let source = r#"
const x: number = 1;
const y: string = "hello";
function foo(a: number, b: string): void {
    console.log(a, b);
}
"#;
        let result = GLOBALS.set(&Globals::default(), || {
            transform_typescript(Path::new("test.ts"), source, "ts")
        });

        assert!(result.is_ok());
        let code = result.unwrap();
        assert!(!code.contains(": number"));
        assert!(!code.contains(": string"));
        assert!(!code.contains(": void"));
        assert!(code.contains("console.log"));
    }

    #[test]
    fn test_transform_tsx() {
        let source = r#"
interface Props {
    name: string;
}

const Component = (props: Props) => {
    return <div>Hello {props.name}</div>;
};
"#;
        let result = GLOBALS.set(&Globals::default(), || {
            transform_typescript(Path::new("test.tsx"), source, "tsx")
        });

        assert!(result.is_ok());
        let code = result.unwrap();
        assert!(!code.contains("interface Props"));
        assert!(!code.contains(": Props"));
    }
}
