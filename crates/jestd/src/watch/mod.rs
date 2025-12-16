use anyhow::Result;
use notify::{Config, Event, RecommendedWatcher, RecursiveMode, Watcher};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tracing::{debug, info};

/// File watcher for watch mode
pub struct FileWatcher {
    watcher: RecommendedWatcher,
    rx: Receiver<Result<Event, notify::Error>>,
    watched_paths: Arc<Mutex<HashSet<PathBuf>>>,
}

impl FileWatcher {
    pub fn new() -> Result<Self> {
        let (tx, rx) = channel();

        let watcher = RecommendedWatcher::new(
            move |res| {
                let _ = tx.send(res);
            },
            Config::default().with_poll_interval(Duration::from_millis(500)),
        )?;

        Ok(Self {
            watcher,
            rx,
            watched_paths: Arc::new(Mutex::new(HashSet::new())),
        })
    }

    /// Watch a directory recursively
    pub fn watch(&mut self, path: &Path) -> Result<()> {
        let path = path.canonicalize()?;
        let mut watched = self.watched_paths.lock().unwrap();

        if !watched.contains(&path) {
            self.watcher.watch(&path, RecursiveMode::Recursive)?;
            watched.insert(path.clone());
            info!("Watching {}", path.display());
        }

        Ok(())
    }

    /// Stop watching a directory
    pub fn unwatch(&mut self, path: &Path) -> Result<()> {
        let path = path.canonicalize()?;
        let mut watched = self.watched_paths.lock().unwrap();

        if watched.remove(&path) {
            self.watcher.unwatch(&path)?;
            info!("Stopped watching {}", path.display());
        }

        Ok(())
    }

    /// Get pending file change events (non-blocking)
    pub fn get_changes(&self) -> Vec<PathBuf> {
        let mut changed_files = HashSet::new();

        // Drain all pending events
        while let Ok(event) = self.rx.try_recv() {
            if let Ok(event) = event {
                for path in event.paths {
                    // Only include source files
                    if is_source_file(&path) {
                        debug!("File changed: {}", path.display());
                        changed_files.insert(path);
                    }
                }
            }
        }

        changed_files.into_iter().collect()
    }

    /// Wait for file changes with timeout (blocking)
    pub fn wait_for_changes(&self, timeout: Duration) -> Vec<PathBuf> {
        let mut changed_files = HashSet::new();

        // Wait for first event with timeout
        match self.rx.recv_timeout(timeout) {
            Ok(Ok(event)) => {
                for path in event.paths {
                    if is_source_file(&path) {
                        changed_files.insert(path);
                    }
                }
            }
            _ => return vec![],
        }

        // Drain any additional events that came in (debounce)
        std::thread::sleep(Duration::from_millis(50));
        while let Ok(Ok(event)) = self.rx.try_recv() {
            for path in event.paths {
                if is_source_file(&path) {
                    changed_files.insert(path);
                }
            }
        }

        changed_files.into_iter().collect()
    }
}

fn is_source_file(path: &Path) -> bool {
    if let Some(ext) = path.extension() {
        let ext = ext.to_string_lossy();
        matches!(
            ext.as_ref(),
            "js" | "jsx" | "ts" | "tsx" | "mjs" | "cjs" | "mts" | "cts" | "json"
        )
    } else {
        false
    }
}

/// Determine which test files need to be re-run based on changed files
pub fn get_affected_tests(
    changed_files: &[PathBuf],
    all_test_files: &[PathBuf],
) -> Vec<PathBuf> {
    let mut affected = HashSet::new();

    for changed in changed_files {
        let changed_stem = changed.file_stem().and_then(|s| s.to_str()).unwrap_or("");

        // If it's a test file itself, add it
        if is_test_file(changed) {
            affected.insert(changed.clone());
            continue;
        }

        // Find test files that might test this source file
        for test_file in all_test_files {
            let test_stem = test_file
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("");

            // Remove .test or .spec suffix
            let base_name = test_stem
                .trim_end_matches(".test")
                .trim_end_matches(".spec");

            // Check if test file matches the changed source file
            if base_name == changed_stem {
                affected.insert(test_file.clone());
            }

            // Check if they're in the same directory or related directories
            if let (Some(changed_parent), Some(test_parent)) =
                (changed.parent(), test_file.parent())
            {
                // Same directory
                if changed_parent == test_parent {
                    affected.insert(test_file.clone());
                }

                // Test in __tests__ subdirectory
                if test_parent.ends_with("__tests__")
                    && test_parent.parent() == Some(changed_parent)
                {
                    if base_name == changed_stem {
                        affected.insert(test_file.clone());
                    }
                }
            }
        }
    }

    // If no specific tests found, run all tests
    if affected.is_empty() && !changed_files.is_empty() {
        return all_test_files.to_vec();
    }

    affected.into_iter().collect()
}

fn is_test_file(path: &Path) -> bool {
    let name = path.file_name().and_then(|s| s.to_str()).unwrap_or("");
    name.contains(".test.") || name.contains(".spec.") || name.contains("__tests__")
}
