//! Repository-relative path handling.
//!
//! Every path stored in a RepoDNA artifact is relative to the repository root and uses
//! forward slashes, regardless of the operating system that produced it. Absolute paths
//! are never stored because they can reveal user names and private directory layouts.

use std::path::{Component, Path};

/// Normalizes a repository-relative path.
///
/// Backslashes become forward slashes, `.` segments and duplicate separators are removed,
/// and `..` segments are resolved. Returns `None` for absolute paths and for paths that
/// would escape the repository root, which makes this function a guard against path
/// traversal as well as a formatter.
///
/// ```
/// use repodna_core::paths::normalize_relative;
/// assert_eq!(normalize_relative("./src\\lib.rs").as_deref(), Some("src/lib.rs"));
/// assert_eq!(normalize_relative("a/../b").as_deref(), Some("b"));
/// assert_eq!(normalize_relative("../outside"), None);
/// ```
pub fn normalize_relative(path: &str) -> Option<String> {
    let unified = path.replace('\\', "/");
    if unified.starts_with('/') || has_windows_prefix(&unified) {
        return None;
    }
    let mut segments: Vec<&str> = Vec::new();
    for segment in unified.split('/') {
        match segment {
            "" | "." => {}
            ".." => {
                segments.pop()?;
            }
            other => segments.push(other),
        }
    }
    Some(segments.join("/"))
}

fn has_windows_prefix(path: &str) -> bool {
    let bytes = path.as_bytes();
    bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':'
}

/// Converts `path` into a repository-relative string if it lies under `root`.
///
/// Returns `None` when `path` is outside `root` or contains components (such as `..`)
/// that cannot be represented safely.
pub fn relative_to(root: &Path, path: &Path) -> Option<String> {
    let relative = path.strip_prefix(root).ok()?;
    let mut segments = Vec::new();
    for component in relative.components() {
        match component {
            Component::Normal(part) => segments.push(part.to_string_lossy().into_owned()),
            Component::CurDir => {}
            _ => return None,
        }
    }
    Some(segments.join("/"))
}

/// Joins `relative` onto the directory `base_dir` (both repository-relative) and normalizes
/// the result. Returns `None` if the result would escape the repository root.
pub fn join(base_dir: &str, relative: &str) -> Option<String> {
    if base_dir.is_empty() {
        normalize_relative(relative)
    } else {
        normalize_relative(&format!("{base_dir}/{relative}"))
    }
}

/// Returns the parent directory of a repository path, or `""` for files at the root.
pub fn parent(path: &str) -> &str {
    path.rsplit_once('/').map_or("", |(dir, _)| dir)
}

/// Returns the final component of a repository path.
pub fn file_name(path: &str) -> &str {
    path.rsplit_once('/').map_or(path, |(_, name)| name)
}

/// Returns the lowercase extension of the file name without the leading dot.
///
/// Dotfiles such as `.gitignore` have no extension.
pub fn extension(path: &str) -> Option<String> {
    let name = file_name(path);
    let (stem, ext) = name.rsplit_once('.')?;
    if stem.is_empty() || ext.is_empty() {
        None
    } else {
        Some(ext.to_ascii_lowercase())
    }
}

/// Returns the file name without its final extension.
pub fn file_stem(path: &str) -> &str {
    let name = file_name(path);
    match name.rsplit_once('.') {
        Some((stem, _)) if !stem.is_empty() => stem,
        _ => name,
    }
}

/// Returns the directories that contain `path`, from the outermost inwards.
///
/// `ancestors("a/b/c.rs")` yields `"a"` and `"a/b"`.
pub fn ancestors(path: &str) -> Vec<&str> {
    path.match_indices('/')
        .map(|(index, _)| &path[..index])
        .collect()
}

/// Returns the first `count` components of `path`.
///
/// `prefix("a/b/c/d.rs", 2)` is `"a/b"`; paths with fewer components are returned unchanged.
pub fn prefix(path: &str, count: usize) -> &str {
    if count == 0 {
        return "";
    }
    match path.match_indices('/').nth(count - 1) {
        Some((index, _)) => &path[..index],
        None => path,
    }
}

/// Returns the number of components in `path` (`0` for the empty root path).
pub fn depth(path: &str) -> usize {
    if path.is_empty() {
        0
    } else {
        path.split('/').count()
    }
}

/// Returns `true` if `path` is `dir` itself or lies inside it. The empty `dir` is the root.
pub fn is_within(path: &str, dir: &str) -> bool {
    dir.is_empty()
        || path == dir
        || (path.len() > dir.len() && path.starts_with(dir) && path.as_bytes()[dir.len()] == b'/')
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn normalizes_separators_and_dots() {
        assert_eq!(
            normalize_relative("src//main.rs").as_deref(),
            Some("src/main.rs")
        );
        assert_eq!(
            normalize_relative("src/./a/../b.rs").as_deref(),
            Some("src/b.rs")
        );
        assert_eq!(
            normalize_relative("docs\\guide.md").as_deref(),
            Some("docs/guide.md")
        );
        assert_eq!(normalize_relative("").as_deref(), Some(""));
    }

    #[test]
    fn rejects_traversal_and_absolute_paths() {
        assert_eq!(normalize_relative("../etc/passwd"), None);
        assert_eq!(normalize_relative("a/../../b"), None);
        assert_eq!(normalize_relative("/etc/passwd"), None);
        assert_eq!(normalize_relative("C:\\Windows"), None);
        assert_eq!(normalize_relative("\\\\server\\share"), None);
    }

    #[test]
    fn computes_relative_paths_under_root() {
        let root = PathBuf::from("repo");
        let file = root.join("src").join("lib.rs");
        assert_eq!(relative_to(&root, &file).as_deref(), Some("src/lib.rs"));
        assert_eq!(relative_to(&root, &PathBuf::from("elsewhere/x")), None);
        assert_eq!(relative_to(&root, &root).as_deref(), Some(""));
    }

    #[test]
    fn joins_within_the_repository() {
        assert_eq!(
            join("src/app", "../lib/util.ts").as_deref(),
            Some("src/lib/util.ts")
        );
        assert_eq!(join("", "./main.py").as_deref(), Some("main.py"));
        assert_eq!(join("src", "../../escape"), None);
    }

    #[test]
    fn splits_components() {
        assert_eq!(parent("a/b/c.rs"), "a/b");
        assert_eq!(parent("c.rs"), "");
        assert_eq!(file_name("a/b/c.rs"), "c.rs");
        assert_eq!(file_stem("a/b/c.test.ts"), "c.test");
        assert_eq!(file_stem(".gitignore"), ".gitignore");
        assert_eq!(extension("a/B.RS").as_deref(), Some("rs"));
        assert_eq!(extension(".gitignore"), None);
        assert_eq!(extension("Makefile"), None);
        assert_eq!(ancestors("a/b/c.rs"), vec!["a", "a/b"]);
        assert!(ancestors("c.rs").is_empty());
        assert_eq!(prefix("a/b/c/d.rs", 2), "a/b");
        assert_eq!(prefix("a/b", 5), "a/b");
        assert_eq!(prefix("a/b", 0), "");
        assert_eq!(depth("a/b/c"), 3);
        assert_eq!(depth(""), 0);
    }

    #[test]
    fn checks_containment_on_component_boundaries() {
        assert!(is_within("src/lib.rs", "src"));
        assert!(is_within("src", "src"));
        assert!(!is_within("srcx/lib.rs", "src"));
        assert!(is_within("anything", ""));
    }
}
