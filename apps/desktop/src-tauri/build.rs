//! Generates the Tauri context (configuration, icons, and permissions) at build time.

fn main() {
    tauri_build::build();
}
