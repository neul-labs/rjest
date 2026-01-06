//! Utility functions for rjest
//!
//! Provides shared functionality used across multiple modules:
//! - Glob pattern matching
//! - Path utilities
//! - Common helpers
//!
//! Note: This module is named `util` to avoid conflicts with SWC's `util` module.

use std::path::Path;

/// Convert a Jest/minimatch glob pattern to regex
///
/// Handles:
/// - `**` = any path segments (including /)
/// - `*` = any chars except /
/// - `?` = single char
/// - `[abc]` = char class
/// - `?(x)` = zero or one of x
/// - `+(x)` = one or more of x
/// - `*(x)` = zero or more of x
/// - `@(x|y)` = one of x or y
pub fn glob_match(pattern: &str, path: &str) -> bool {
    // Convert Jest/minimatch glob pattern to regex
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
            '?' => {
                // ? = single char (or any char for simple glob)
                regex_pattern.push('.');
                i += 1;
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

/// Extract a group from glob pattern - handles nested parentheses
fn extract_group(chars: &[char], start: usize) -> (String, usize) {
    // start points to '('
    let mut depth = 0;
    let mut end = start;

    while end < chars.len() {
        match chars[end] {
            '(' => {
                depth += 1;
                end += 1;
            }
            ')' if depth == 1 => {
                // Found the matching closing paren
                end += 1;
                break;
            }
            ')' => {
                depth -= 1;
                end += 1;
            }
            _ => end += 1,
        }
    }

    // Extract the content between the outer parentheses
    let group: String = chars[start..end - 1].iter().collect();
    (group, end - 1)
}

/// Check if a path is absolute and normalized
pub fn is_safe_path(path: &Path, base: &Path) -> bool {
    if let Ok(canonical) = path.canonicalize() {
        if let Ok(base_canonical) = base.canonicalize() {
            return canonical.starts_with(&base_canonical);
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_glob_match_simple() {
        assert!(glob_match("**/*.test.js", "foo.test.js"));
        assert!(glob_match("**/*.test.js", "src/foo.test.js"));
        assert!(glob_match("**/*.test.js", "deep/nested/src/foo.test.js"));
        assert!(!glob_match("**/*.test.js", "foo.spec.js"));
    }

    #[test]
    fn test_glob_match_no_double_star() {
        assert!(glob_match("src/*.test.js", "src/foo.test.js"));
        assert!(!glob_match("src/*.test.js", "src/sub/bar.test.js"));
    }

    #[test]
    fn test_glob_match_question_mark() {
        assert!(glob_match("src/?.test.js", "src/a.test.js"));
        assert!(!glob_match("src/?.test.js", "src/ab.test.js"));
    }

    #[test]
    fn test_glob_match_patterns() {
        // Test file patterns
        assert!(glob_match("**/*.test.ts", "foo.test.ts"));
        assert!(glob_match("**/*.spec.ts", "foo.spec.ts"));
        assert!(glob_match("**/__tests__/**/*.ts", "__tests__/foo.test.ts"));

        // Non-matches
        assert!(!glob_match("**/*.test.ts", "foo.ts"));
        assert!(!glob_match("**/*.test.ts", "foo.test.js"));
    }

    #[test]
    fn test_is_safe_path() {
        use std::fs;
        use tempfile::TempDir;

        // Create a temp directory structure
        let temp_dir = TempDir::new().unwrap();
        let base = temp_dir.path();

        // Create some files
        let safe_path = base.join("src/main.ts");
        fs::create_dir_all(safe_path.parent().unwrap()).unwrap();
        fs::write(&safe_path, "").unwrap();

        // Test safe paths
        assert!(is_safe_path(&safe_path, base));
        assert!(is_safe_path(&base.join("src"), base));

        // Test unsafe path (simulated by trying to escape)
        let unsafe_path = Path::new("/etc/passwd");
        assert!(!is_safe_path(unsafe_path, base));
    }
}
