use std::path::PathBuf;
use std::process::Command;

fn main() {
    let manifest = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"));
    let root = manifest.join("../..");
    let js = root.join("crates/secd-ui/dist/secd-ui.js");
    let wasm = root.join("crates/secd-ui/dist/secd-ui.wasm");
    println!("cargo:rerun-if-changed={}", js.display());
    println!("cargo:rerun-if-changed={}", wasm.display());
    println!(
        "cargo:rerun-if-changed={}",
        root.join("crates/secd-ui/src").display()
    );
    if js.is_file() && wasm.is_file() {
        return;
    }
    let script = root.join("scripts/build-ui.sh");
    let status = Command::new(&script)
        .status()
        .unwrap_or_else(|e| panic!("run {}: {e}", script.display()));
    assert!(status.success(), "scripts/build-ui.sh failed");
    assert!(js.is_file(), "missing {}", js.display());
    assert!(wasm.is_file(), "missing {}", wasm.display());
}
