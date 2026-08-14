#!/usr/bin/env bash
# Build secd-ui wasm + wasm-bindgen loader into crates/secd-ui/dist/.
set -euo pipefail
root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$root"
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$root/target}"
PATH="$HOME/.cargo/bin:$PATH"

cargo build --release --target wasm32-unknown-unknown \
  --manifest-path "$root/crates/secd-ui/Cargo.toml" \
  --no-default-features --features csr --bin secd-ui

wasm="${CARGO_TARGET_DIR}/wasm32-unknown-unknown/release/secd-ui.wasm"
mkdir -p "$root/crates/secd-ui/dist"
wasm-bindgen --target web --no-typescript --out-dir "$root/crates/secd-ui/dist" "$wasm"
# Serve a single wasm name; the generated loader is rewritten to fetch it.
if [[ -f "$root/crates/secd-ui/dist/secd-ui_bg.wasm" ]]; then
  mv "$root/crates/secd-ui/dist/secd-ui_bg.wasm" "$root/crates/secd-ui/dist/secd-ui.wasm"
fi
if command -v wasm-opt >/dev/null 2>&1; then
  wasm-opt -Oz --enable-bulk-memory --enable-nontrapping-float-to-int \
    -o "$root/crates/secd-ui/dist/secd-ui.wasm.opt" "$root/crates/secd-ui/dist/secd-ui.wasm"
  mv "$root/crates/secd-ui/dist/secd-ui.wasm.opt" "$root/crates/secd-ui/dist/secd-ui.wasm"
fi
echo "build-ui: crates/secd-ui/dist/secd-ui.wasm"
