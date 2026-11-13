use std::path::PathBuf;
use std::process::Command;

fn main() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let icon = manifest_dir.join("icons/icon.png");
    println!("cargo:rerun-if-changed={}", icon.display());

    // Tauri embeds icons/icon.png while compiling, including when bundle.active is false.
    // A fresh clone may not have run scripts/gen-icon.mjs yet.
    if !icon.is_file() {
        let script = manifest_dir.join("../scripts/gen-icon.mjs");
        let status = Command::new("node")
            .arg(&script)
            .status()
            .unwrap_or_else(|err| {
                panic!(
                    "src-tauri/icons/icon.png is missing and node could not run ({err}). From the repo root: node scripts/gen-icon.mjs"
                )
            });
        if !status.success() || !icon.is_file() {
            panic!(
                "src-tauri/icons/icon.png is missing. From the repo root: node scripts/gen-icon.mjs"
            );
        }
    }

    tauri_build::build()
}
