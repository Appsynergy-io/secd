#!/usr/bin/env bash
set -euo pipefail
root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$root"
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$root/target}"

"$root/scripts/build-ui.sh"

cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo test --workspace --release

probe="$(mktemp -d)"
trap 'rm -rf "$probe"' EXIT
mkdir -p "$probe/src"
cat >"$probe/Cargo.toml" <<EOF
[package]
name = "secd-compile-fail"
version = "0.0.0"
edition = "2021"
publish = false

[dependencies]
secd-core = { path = "$root/crates/secd-core" }
serde = "=1.0.229"
EOF
cp "$root/tests/compile-fail/secret_is_not_printable.rs" "$probe/src/main.rs"
set +e
cargo build --manifest-path "$probe/Cargo.toml" --quiet >"$probe/out" 2>&1
status=$?
set -e
if [[ "$status" -eq 0 ]]; then
  echo "compile-fail: secret_is_not_printable.rs compiled; Secret must not implement Display/Serialize/Deref" >&2
  exit 1
fi
for trait in Display Serialize Deref; do
  if ! grep -q "$trait" "$probe/out"; then
    echo "compile-fail: expected $trait error" >&2
    cat "$probe/out" >&2
    exit 1
  fi
done

"$root/scripts/plan-contract.sh"
