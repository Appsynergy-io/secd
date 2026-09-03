#!/usr/bin/env bash
# Build the bun console into ui/dist/.
#
# bun is pinned by version and sha256 per platform in scripts/tools.sh.
# HTML stays index.html. Split JS/CSS include a content hash so the
# runtime helper and crypto.ts do not collide; two builds of the same
# tree still compare equal.
set -euo pipefail
root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$root"
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$root/target}"
export CARGO_HOME="${CARGO_HOME:-$HOME/.cargo}"
PATH="$CARGO_HOME/bin:$HOME/.cargo/bin:$PATH"
export PATH
export SOURCE_DATE_EPOCH="${SOURCE_DATE_EPOCH:-0}"
export TZ=UTC
export DO_NOT_TRACK=1

SECD_ROOT="$root"
SECD_TOOL_TAG="build-ui"
export SECD_ROOT SECD_TOOL_TAG
# shellcheck source=scripts/tools.sh
. "$root/scripts/tools.sh"

secd_ensure_bun
[[ "$(bun --version 2>/dev/null)" == "$BUN_PINNED" ]] \
  || secd_die "bun $(bun --version 2>/dev/null) is not pinned ${BUN_PINNED}"

# The sidebar's version chip is the workspace version, bound at build time.
version="$(sed -nE 's/^version = "([^"]+)"/\1/p' "$root/Cargo.toml" | head -n 1)"
[[ -n "$version" ]] || secd_die "build-ui: no version in Cargo.toml"

ui="$root/ui"
cd "$ui"
bun install --frozen-lockfile --ignore-scripts
rm -rf dist
mkdir -p dist
# The key holder is its own entry point: the bundler emits a chunk for
# `new Worker(...)` but not for `new SharedWorker(...)`, and --entry-naming
# leaves this one unhashed so the page can name it.
bun build ./index.html ./src/keyholder.worker.ts \
  --outdir dist \
  --target browser \
  --production \
  --sourcemap=none \
  --splitting \
  --entry-naming "[name].[ext]" \
  --chunk-naming "[name]-[hash].[ext]" \
  --asset-naming "[name].[ext]" \
  --external="*.woff2" \
  --define "SECD_VERSION=\"$version\""
mkdir -p dist/fonts
cp -a fonts/*.woff2 fonts/OFL.txt dist/fonts/
[[ -f dist/fonts/geist-latin-wght-normal.woff2 && -f dist/fonts/geist-mono-latin-wght-normal.woff2 ]] \
  || secd_die "build-ui: missing Geist faces in dist/fonts"
echo "build-ui: ui/dist"
