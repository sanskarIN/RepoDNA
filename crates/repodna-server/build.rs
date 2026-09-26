//! Embeds the built web interface (`apps/web/dist`, or the directory named by
//! `REPODNA_WEB_DIST`) when it exists and the `embed-web` feature is on, so release builds
//! of `repodna serve` include it. Without it, the server falls back to a small built-in page.

use std::env;
use std::fs;
use std::path::{Path, PathBuf};

fn collect(dir: &Path, base: &Path, out: &mut Vec<(String, PathBuf)>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    let mut paths: Vec<PathBuf> = entries.filter_map(Result::ok).map(|e| e.path()).collect();
    paths.sort();
    for path in paths {
        if path.is_dir() {
            collect(&path, base, out);
        } else if let Ok(relative) = path.strip_prefix(base) {
            out.push((relative.to_string_lossy().replace('\\', "/"), path));
        }
    }
}

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-env-changed=REPODNA_WEB_DIST");
    let manifest =
        PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("Cargo sets CARGO_MANIFEST_DIR"));
    let chosen = env::var_os("REPODNA_WEB_DIST").map(PathBuf::from);
    let web = manifest.join("../../apps/web");
    let dist = chosen.clone().unwrap_or_else(|| web.join("dist"));
    let mut files = Vec::new();
    let embed = env::var_os("CARGO_FEATURE_EMBED_WEB").is_some();
    if embed && dist.join("index.html").is_file() {
        println!("cargo:rerun-if-changed={}", dist.display());
        collect(&dist, &dist, &mut files);
    } else if embed && chosen.is_none() && web.is_dir() {
        // Run again when the web interface is built later.
        println!("cargo:rerun-if-changed={}", web.display());
    }
    let mut code = String::from(
        "/// Files of the built web interface, as `(path, contents)`; empty when the build did not include it.\npub static WEB_ASSETS: &[(&str, &[u8])] = &[\n",
    );
    for (name, path) in &files {
        let absolute = path.canonicalize().unwrap_or_else(|_| path.clone());
        code.push_str(&format!(
            "    ({name:?}, include_bytes!({:?})),\n",
            absolute.to_string_lossy()
        ));
    }
    code.push_str("];\n");
    let out = PathBuf::from(env::var("OUT_DIR").expect("Cargo sets OUT_DIR")).join("web_assets.rs");
    fs::write(out, code).expect("the generated asset list can be written");
}
