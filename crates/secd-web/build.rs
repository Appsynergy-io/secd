//! Guard the generated web console rather than build it.
//!
//! This used to shell out to scripts/build-ui.sh when dist/ was missing, which
//! runs `cargo build` against the same CARGO_TARGET_DIR the parent cargo
//! already holds an exclusive lock on: a deadlock until the job times out. It
//! also returned early whenever dist/ merely *existed*, so a stale console was
//! silently embedded in the server binary.
//!
//! scripts/check.sh and scripts/release.sh both build the UI before touching
//! this crate. If dist/ is missing or older than its inputs, say so and stop.

use std::path::{Path, PathBuf};
use std::time::SystemTime;

fn main() {
    let manifest = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"));
    let root = manifest.join("../..");
    let js = root.join("crates/secd-ui/dist/secd-ui.js");
    let wasm = root.join("crates/secd-ui/dist/secd-ui.wasm");

    let inputs = [
        root.join("crates/secd-ui/src"),
        root.join("vendor/appsy-ui/src"),
        root.join("Cargo.lock"),
        root.join("scripts/build-ui.sh"),
    ];
    for path in [&js, &wasm].into_iter().chain(inputs.iter()) {
        println!("cargo:rerun-if-changed={}", path.display());
    }

    if !js.is_file() || !wasm.is_file() {
        panic!(
            "missing crates/secd-ui/dist — run scripts/build-ui.sh (or scripts/check.sh ui) first"
        );
    }

    let built = newest(&wasm).min(newest(&js));
    for path in &inputs {
        let changed = newest(path);
        if changed > built {
            panic!(
                "crates/secd-ui/dist is older than {} — run scripts/build-ui.sh \
                 (or scripts/check.sh ui) to rebuild the console",
                path.display()
            );
        }
    }
}

/// Newest mtime at or under `path`. Missing paths sort oldest.
fn newest(path: &Path) -> SystemTime {
    let Ok(meta) = std::fs::metadata(path) else {
        return SystemTime::UNIX_EPOCH;
    };
    if !meta.is_dir() {
        return meta.modified().unwrap_or(SystemTime::UNIX_EPOCH);
    }
    let Ok(entries) = std::fs::read_dir(path) else {
        return SystemTime::UNIX_EPOCH;
    };
    entries
        .filter_map(Result::ok)
        .map(|e| newest(&e.path()))
        .max()
        .unwrap_or(SystemTime::UNIX_EPOCH)
}
