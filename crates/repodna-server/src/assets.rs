//! The web interface: embedded at build time, read from a directory, or absent (the server
//! then shows a small built-in page with links to reports).

use std::path::{Path, PathBuf};

use repodna_core::paths::normalize_relative;

mod embedded {
    include!(concat!(env!("OUT_DIR"), "/web_assets.rs"));
}

/// Largest file served from a web directory.
const MAX_ASSET_BYTES: u64 = 32 * 1024 * 1024;

/// Where the web interface comes from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Assets {
    /// Files embedded in this build.
    Embedded,
    /// Files in a directory (for example `apps/web/dist` during development).
    Directory(PathBuf),
    /// No web interface: the built-in page is used.
    Builtin,
}

/// Content type for a file name.
pub fn content_type(path: &str) -> &'static str {
    let extension = path
        .rsplit_once('.')
        .map(|(_, ext)| ext.to_ascii_lowercase());
    match extension.as_deref() {
        Some("html") => "text/html; charset=utf-8",
        Some("js" | "mjs") => "text/javascript; charset=utf-8",
        Some("css") => "text/css; charset=utf-8",
        Some("json" | "map") => "application/json; charset=utf-8",
        Some("webmanifest") => "application/manifest+json",
        Some("svg") => "image/svg+xml",
        Some("png") => "image/png",
        Some("ico") => "image/x-icon",
        Some("woff2") => "font/woff2",
        Some("txt") => "text/plain; charset=utf-8",
        _ => "application/octet-stream",
    }
}

impl Assets {
    /// Uses `directory` when given, otherwise the embedded files when this build has them.
    pub fn choose(directory: Option<PathBuf>) -> Self {
        match directory {
            Some(directory) => Self::Directory(directory),
            None if !embedded::WEB_ASSETS.is_empty() => Self::Embedded,
            None => Self::Builtin,
        }
    }

    /// `true` when a web interface is available.
    pub fn available(&self) -> bool {
        *self != Self::Builtin
    }

    fn read(&self, relative: &str) -> Option<Vec<u8>> {
        match self {
            Self::Embedded => embedded::WEB_ASSETS
                .iter()
                .find(|(name, _)| *name == relative)
                .map(|(_, bytes)| bytes.to_vec()),
            Self::Directory(root) => read_file(root, relative),
            Self::Builtin => None,
        }
    }

    /// The file for a request path, falling back to `index.html` for paths without an
    /// extension (client-side routes). Returns the bytes and their content type.
    pub fn get(&self, path: &str) -> Option<(Vec<u8>, &'static str)> {
        let relative = normalize_relative(path.trim_start_matches('/'))?;
        let relative = if relative.is_empty() {
            "index.html".to_owned()
        } else {
            relative
        };
        if let Some(bytes) = self.read(&relative) {
            return Some((bytes, content_type(&relative)));
        }
        let last = relative.rsplit('/').next().unwrap_or_default();
        if last.contains('.') {
            return None;
        }
        self.read("index.html")
            .map(|bytes| (bytes, content_type("index.html")))
    }
}

fn read_file(root: &Path, relative: &str) -> Option<Vec<u8>> {
    let path = relative
        .split('/')
        .fold(root.to_path_buf(), |path, part| path.join(part));
    let metadata = std::fs::symlink_metadata(&path).ok()?;
    if !metadata.is_file() || metadata.len() > MAX_ASSET_BYTES {
        return None;
    }
    std::fs::read(path).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serves_files_and_client_routes_from_a_directory() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("assets")).unwrap();
        std::fs::write(dir.path().join("index.html"), "<!doctype html>").unwrap();
        std::fs::write(dir.path().join("assets/app.js"), "console.log(1)").unwrap();
        let assets = Assets::choose(Some(dir.path().to_path_buf()));
        assert!(assets.available());
        let (bytes, kind) = assets.get("/assets/app.js").unwrap();
        assert_eq!(bytes, b"console.log(1)");
        assert_eq!(kind, "text/javascript; charset=utf-8");
        assert_eq!(assets.get("/").unwrap().1, "text/html; charset=utf-8");
        assert_eq!(
            assets.get("/repositories/abc").unwrap().0,
            b"<!doctype html>"
        );
        assert!(assets.get("/assets/missing.js").is_none());
        assert!(assets.get("/../secret.txt").is_none());
        assert_eq!(content_type("x.woff2"), "font/woff2");
        assert!(!Assets::Builtin.available());
    }
}
