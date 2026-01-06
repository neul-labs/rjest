use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use std::process::Command;
use tracing::{debug, info, warn};

/// Find the git root directory by walking up from the given path
fn find_git_root(start: &Path) -> Option<PathBuf> {
    let mut current = start.to_path_buf();

    // First try to canonicalize
    if let Ok(canonical) = current.canonicalize() {
        current = canonical;
    }

    loop {
        if current.join(".git").exists() {
            return Some(current);
        }
        if !current.pop() {
            return None;
        }
    }
}

/// Get list of files changed since the last commit or compared to a branch
pub fn get_changed_files(project_root: &Path) -> Result<Vec<PathBuf>> {
    // Find the git root (walk up from project_root)
    let git_root = find_git_root(project_root);
    if git_root.is_none() {
        debug!("Not a git repository: {}", project_root.display());
        return Ok(vec![]);
    }
    let git_root = git_root.unwrap();

    let mut changed_files = Vec::new();

    // Get uncommitted changes (staged + unstaged)
    let uncommitted = get_uncommitted_changes(&git_root)?;
    changed_files.extend(uncommitted);

    // Get files changed since merge-base with main/master
    let since_base = get_changes_since_merge_base(&git_root)?;
    changed_files.extend(since_base);

    // Filter to only files within project_root
    let project_root_canonical = project_root.canonicalize().unwrap_or(project_root.to_path_buf());
    changed_files.retain(|f| {
        match f.canonicalize() {
            Ok(canonical) => canonical.starts_with(&project_root_canonical),
            Err(e) => {
                warn!("Failed to canonicalize changed file {}: {}", f.display(), e);
                false
            }
        }
    });

    // Deduplicate
    changed_files.sort();
    changed_files.dedup();

    info!("Found {} changed files", changed_files.len());
    for file in &changed_files {
        debug!("Changed: {}", file.display());
    }

    Ok(changed_files)
}

/// Get uncommitted changes (both staged and unstaged)
fn get_uncommitted_changes(project_root: &Path) -> Result<Vec<PathBuf>> {
    // Validate the path before passing to Command
    if !project_root.exists() || !project_root.is_dir() {
        warn!("Invalid project root for git command: {}", project_root.display());
        return Ok(vec![]);
    }

    let output = Command::new("git")
        .args(["status", "--porcelain", "-uall"])
        .current_dir(project_root)
        .output()
        .context("Failed to run git status")?;

    if !output.status.success() {
        return Ok(vec![]);
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let files: Vec<PathBuf> = stdout
        .lines()
        .filter_map(|line| {
            // Format: XY filename or XY original -> renamed
            if line.len() > 3 {
                let file_part = &line[3..];
                // Handle renames (take the new name)
                let filename = if file_part.contains(" -> ") {
                    file_part.split(" -> ").last().unwrap_or(file_part)
                } else {
                    file_part
                };
                Some(project_root.join(filename.trim()))
            } else {
                None
            }
        })
        .filter(|p| p.exists())
        .collect();

    Ok(files)
}

/// Get files changed since merge-base with main/master branch
fn get_changes_since_merge_base(project_root: &Path) -> Result<Vec<PathBuf>> {
    // Validate the path before passing to Command
    if !project_root.exists() || !project_root.is_dir() {
        warn!("Invalid project root for git command: {}", project_root.display());
        return Ok(vec![]);
    }

    // Try to find the main branch
    let main_branch = get_main_branch(project_root)?;

    if main_branch.is_none() {
        return Ok(vec![]);
    }
    let main_branch = main_branch.unwrap();

    // Get current branch
    let current_branch = get_current_branch(project_root)?;

    // If we're on main, compare to HEAD~1
    let compare_to = if current_branch.as_deref() == Some(&main_branch) {
        "HEAD~1".to_string()
    } else {
        // Find merge-base with main branch
        let output = Command::new("git")
            .args(["merge-base", "HEAD", &main_branch])
            .current_dir(project_root)
            .output();

        match output {
            Ok(o) if o.status.success() => {
                String::from_utf8_lossy(&o.stdout).trim().to_string()
            }
            _ => return Ok(vec![]),
        }
    };

    // Get changed files since merge-base
    let output = Command::new("git")
        .args(["diff", "--name-only", &compare_to, "HEAD"])
        .current_dir(project_root)
        .output()
        .context("Failed to run git diff")?;

    if !output.status.success() {
        return Ok(vec![]);
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let files: Vec<PathBuf> = stdout
        .lines()
        .map(|line| project_root.join(line.trim()))
        .filter(|p| p.exists())
        .collect();

    Ok(files)
}

/// Get the main branch name (main, master, or default)
fn get_main_branch(project_root: &Path) -> Result<Option<String>> {
    // Check for common main branch names
    for branch in &["main", "master"] {
        let output = Command::new("git")
            .args(["rev-parse", "--verify", branch])
            .current_dir(project_root)
            .output();

        if let Ok(o) = output {
            if o.status.success() {
                return Ok(Some(branch.to_string()));
            }
        }
    }

    // Try to get default branch from remote
    let output = Command::new("git")
        .args(["symbolic-ref", "refs/remotes/origin/HEAD"])
        .current_dir(project_root)
        .output();

    if let Ok(o) = output {
        if o.status.success() {
            let stdout = String::from_utf8_lossy(&o.stdout);
            if let Some(branch) = stdout.trim().strip_prefix("refs/remotes/origin/") {
                return Ok(Some(branch.to_string()));
            }
        }
    }

    Ok(None)
}

/// Get the current branch name
fn get_current_branch(project_root: &Path) -> Result<Option<String>> {
    let output = Command::new("git")
        .args(["rev-parse", "--abbrev-ref", "HEAD"])
        .current_dir(project_root)
        .output()
        .context("Failed to get current branch")?;

    if output.status.success() {
        let branch = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if branch != "HEAD" {
            return Ok(Some(branch));
        }
    }

    Ok(None)
}

/// Filter source files to find related test files
pub fn find_related_test_files(
    changed_files: &[PathBuf],
    all_test_files: &[PathBuf],
) -> Vec<PathBuf> {
    let mut related = Vec::new();

    for test_file in all_test_files {
        // Check if the test file itself was changed
        if changed_files.contains(test_file) {
            related.push(test_file.clone());
            continue;
        }

        // Check if any changed file might be imported by this test
        let test_dir = test_file.parent();
        let test_stem = test_file
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .trim_end_matches(".test")
            .trim_end_matches(".spec");

        for changed in changed_files {
            // Skip if the changed file is not a source file
            let ext = changed.extension().and_then(|e| e.to_str()).unwrap_or("");
            if !matches!(ext, "js" | "jsx" | "ts" | "tsx" | "mjs" | "cjs") {
                continue;
            }

            let changed_stem = changed
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("");

            // Check if names match (foo.ts -> foo.test.ts)
            if changed_stem == test_stem {
                related.push(test_file.clone());
                break;
            }

            // Check if in same directory or parent directory
            if let (Some(changed_dir), Some(test_dir)) = (changed.parent(), test_dir) {
                if changed_dir == test_dir
                    || test_dir.starts_with(changed_dir)
                    || changed_dir.starts_with(test_dir)
                {
                    related.push(test_file.clone());
                    break;
                }

                // Check for __tests__ directory relationship
                if test_dir.ends_with("__tests__") {
                    if let Some(test_parent) = test_dir.parent() {
                        if changed_dir == test_parent {
                            related.push(test_file.clone());
                            break;
                        }
                    }
                }
            }
        }
    }

    // Deduplicate
    related.sort();
    related.dedup();

    related
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_find_related_test_files() {
        let changed = vec![
            PathBuf::from("/project/src/utils.ts"),
            PathBuf::from("/project/src/helper.ts"),
        ];

        let tests = vec![
            PathBuf::from("/project/src/utils.test.ts"),
            PathBuf::from("/project/src/other.test.ts"),
            PathBuf::from("/project/src/__tests__/helper.test.ts"),
        ];

        let related = find_related_test_files(&changed, &tests);

        // utils.test.ts should match utils.ts
        assert!(related.contains(&PathBuf::from("/project/src/utils.test.ts")));
    }

    #[test]
    fn test_get_changed_files_non_git_directory() {
        use tempfile::TempDir;
        let temp_dir = TempDir::new().unwrap();
        let result = get_changed_files(temp_dir.path());
        // Non-git directory should return empty vec
        assert!(result.unwrap().is_empty());
    }

    #[test]
    fn test_get_changed_files_nonexistent_directory() {
        let nonexistent = PathBuf::from("/tmp/this-does-not-exist-12345");
        let result = get_changed_files(&nonexistent);
        // Non-existent directory should return empty vec (not an error)
        assert!(result.unwrap().is_empty());
    }

    #[test]
    fn test_find_related_test_files_empty_inputs() {
        let changed = vec![];
        let tests = vec![];
        let related = find_related_test_files(&changed, &tests);
        assert!(related.is_empty());
    }

    #[test]
    fn test_find_related_test_files_only_source_files() {
        let changed = vec![
            PathBuf::from("/project/src/utils.ts"),
        ];
        let tests = vec![];
        let related = find_related_test_files(&changed, &tests);
        assert!(related.is_empty());
    }

    #[test]
    fn test_find_related_test_files_only_tests() {
        let changed = vec![];
        let tests = vec![
            PathBuf::from("/project/src/utils.test.ts"),
        ];
        let related = find_related_test_files(&changed, &tests);
        assert!(related.is_empty());
    }
}
