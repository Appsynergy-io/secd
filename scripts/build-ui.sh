#!/usr/bin/env bash
# Build secd-ui wasm + wasm-bindgen loader into crates/secd-ui/dist/.
#
# Both tools are pinned and verified. wasm-bindgen-cli must match the
# wasm-bindgen crate the lockfile resolved, or the generated glue fails at
# runtime in the browser rather than at build time. wasm-opt is mandatory:
# when it was applied only "if command -v wasm-opt", CI and a developer
# machine produced different wasm, and that wasm is embedded in the shipped
# secd-web binary.
set -euo pipefail
root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$root"
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$root/target}"
export CARGO_HOME="${CARGO_HOME:-$HOME/.cargo}"
PATH="$CARGO_HOME/bin:$HOME/.cargo/bin:$PATH"
export PATH

SECD_ROOT="$root"
SECD_TOOL_TAG="build-ui"
export SECD_ROOT SECD_TOOL_TAG
# shellcheck source=scripts/tools.sh
. "$root/scripts/tools.sh"

# Prebuilt wasm-bindgen-cli is published only for the version below. Any other
# lockfile version falls back to a source build rather than a stale checksum.
WASM_BINDGEN_PINNED="0.2.126"
BINARYEN_PINNED="version_132"

ensure_wasm_bindgen() {
  local ver="$1"
  if command -v wasm-bindgen >/dev/null 2>&1 \
    && [[ "$(wasm-bindgen --version 2>/dev/null | awk '{print $2}')" == "$ver" ]]; then
    return 0
  fi
  if [[ "$ver" != "$WASM_BINDGEN_PINNED" ]]; then
    echo "build-ui: no pinned prebuilt for wasm-bindgen ${ver}; building from source" >&2
    cargo install wasm-bindgen-cli --version "$ver" --locked
    return 0
  fi
  local asset sha
  case "$(uname -s):$(uname -m)" in
    Linux:x86_64)
      asset="wasm-bindgen-${ver}-x86_64-unknown-linux-musl"
      sha="064948d58e2d6c0a745216477a639ba696216d6309aaa902939d1b865b1d869d"
      ;;
    Darwin:arm64 | Darwin:aarch64)
      asset="wasm-bindgen-${ver}-aarch64-apple-darwin"
      sha="7df536babe345deb68828148dbdc71179118afdab42d83547c7cebfbf1426bd5"
      ;;
    *)
      echo "build-ui: no pinned prebuilt for $(uname -s) $(uname -m); building from source" >&2
      cargo install wasm-bindgen-cli --version "$ver" --locked
      return 0
      ;;
  esac
  secd_fetch_tool \
    "https://github.com/rustwasm/wasm-bindgen/releases/download/${ver}/${asset}.tar.gz" \
    "$sha" "${asset}/wasm-bindgen" wasm-bindgen
}

ensure_wasm_opt() {
  local want="${BINARYEN_PINNED#version_}"
  if command -v wasm-opt >/dev/null 2>&1 \
    && [[ "$(wasm-opt --version 2>/dev/null | awk '{print $3}')" == "$want" ]]; then
    return 0
  fi
  local asset sha
  case "$(uname -s):$(uname -m)" in
    Linux:x86_64)
      asset="x86_64-linux"
      sha="195ddc94f9bc89f45abdabb0b9eea86023d727ba90eac8b35b80f2544fc30572"
      ;;
    Darwin:arm64 | Darwin:aarch64)
      asset="arm64-macos"
      sha="98aad827847af7ef990ed7098d885725c8e5b5aae75073403635617ae4e259aa"
      ;;
    Darwin:x86_64)
      asset="x86_64-macos"
      sha="40c3de90bb3766bd0282a895e139a6f50253dba49b4f5bb89e66faca162d832e"
      ;;
    *)
      secd_die "no pinned binaryen for $(uname -s) $(uname -m)"
      ;;
  esac
  secd_fetch_tool \
    "https://github.com/WebAssembly/binaryen/releases/download/${BINARYEN_PINNED}/binaryen-${BINARYEN_PINNED}-${asset}.tar.gz" \
    "$sha" "binaryen-${BINARYEN_PINNED}/bin/wasm-opt" wasm-opt
}

wb_ver="$(secd_lock_version wasm-bindgen)"
[[ -n "$wb_ver" ]] || secd_die "could not read the wasm-bindgen version from Cargo.lock"

if ! rustup target list --installed | grep -qx wasm32-unknown-unknown; then
  rustup target add wasm32-unknown-unknown
fi
ensure_wasm_bindgen "$wb_ver"
ensure_wasm_opt

cargo build --locked --release --target wasm32-unknown-unknown \
  --manifest-path "$root/crates/secd-ui/Cargo.toml" \
  --no-default-features --features csr --bin secd-ui

wasm="${CARGO_TARGET_DIR}/wasm32-unknown-unknown/release/secd-ui.wasm"
dist="$root/crates/secd-ui/dist"
mkdir -p "$dist"
wasm-bindgen --target web --no-typescript --out-dir "$dist" "$wasm"
# Serve a single wasm name; the generated loader is rewritten to fetch it.
if [[ -f "$dist/secd-ui_bg.wasm" ]]; then
  mv "$dist/secd-ui_bg.wasm" "$dist/secd-ui.wasm"
fi
wasm-opt -Oz --enable-bulk-memory --enable-nontrapping-float-to-int \
  -o "$dist/secd-ui.wasm.opt" "$dist/secd-ui.wasm"
mv "$dist/secd-ui.wasm.opt" "$dist/secd-ui.wasm"
echo "build-ui: crates/secd-ui/dist/secd-ui.wasm"
