#!/usr/bin/env bash
# Build the bun console into ui/dist/.
#
# bun is pinned by version and sha256 per platform in scripts/tools.sh.
# Filenames are stable ([name].[ext]); content hashes would make two
# builds of the same tree compare unequal.
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
SECD_TOOL_TAG="build-ui-bun"
export SECD_ROOT SECD_TOOL_TAG
# shellcheck source=scripts/tools.sh
. "$root/scripts/tools.sh"

secd_ensure_bun
[[ "$(bun --version 2>/dev/null)" == "$BUN_PINNED" ]] \
  || secd_die "bun $(bun --version 2>/dev/null) is not pinned ${BUN_PINNED}"

ui="$root/ui"
cd "$ui"
bun install --frozen-lockfile --ignore-scripts
rm -rf dist
mkdir -p dist
bun build ./index.html \
  --outdir dist \
  --target browser \
  --production \
  --sourcemap=none \
  --entry-naming "[name].[ext]" \
  --chunk-naming "[name].[ext]" \
  --asset-naming "[name].[ext]"
echo "build-ui-bun: ui/dist"
