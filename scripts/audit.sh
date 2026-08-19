#!/usr/bin/env bash
# Report RustSec advisories against Cargo.lock.
#
#   audit.sh [--json OUT]
#
# Pinned by version and sha256 and installed into CARGO_HOME/bin, like every
# other external tool here. It used to be `cargo install cargo-audit --locked`
# inside a workflow job that also held `issues: write`, which is exactly the
# shape the "no job that compiles third-party code holds a write scope"
# invariant exists to forbid: build.rs from cargo-audit's whole dependency tree
# ran next to a token that can write to this repository. A pinned binary
# compiles nothing, so the job is powerless again and the version stops being
# whatever crates.io served that morning.
set -euo pipefail
root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$root"
SECD_ROOT="$root"
SECD_TOOL_TAG="audit"
export SECD_ROOT SECD_TOOL_TAG
export CARGO_HOME="${CARGO_HOME:-$HOME/.cargo}"
PATH="$CARGO_HOME/bin:$PATH"
export PATH
# shellcheck source=scripts/tools.sh
. "$root/scripts/tools.sh"

AUDIT_PINNED="0.22.1"

json=""
while [[ $# -gt 0 ]]; do
  case "$1" in
    --json)
      json="${2:?audit: --json needs a path}"
      shift 2
      ;;
    *) secd_die "usage: audit.sh [--json OUT]" ;;
  esac
done

if ! command -v cargo-audit >/dev/null 2>&1 \
  || [[ "$(cargo-audit audit --version 2>/dev/null | awk '{print $2}')" != "$AUDIT_PINNED" ]]; then
  case "$(uname -s):$(uname -m)" in
    Linux:x86_64)
      triple="x86_64-unknown-linux-musl"
      sha="c32506f338bdcdaef5a17fb9f33abb6ecf9561324cfd34237fd335f9283a1eab"
      ;;
    # RustSec publishes no aarch64 macOS build; the x86_64 one runs under
    # Rosetta, which is what a laptop gets either way.
    Darwin:x86_64 | Darwin:arm64 | Darwin:aarch64)
      triple="x86_64-apple-darwin"
      sha="582d104a2a4bdb127c6bf6d056d89eede40686d11f52e4bc1765132ec99d2fca"
      ;;
    *) secd_die "no pinned cargo-audit for $(uname -s) $(uname -m)" ;;
  esac
  name="cargo-audit-${triple}-v${AUDIT_PINNED}"
  secd_fetch_tool \
    "https://github.com/rustsec/rustsec/releases/download/cargo-audit%2Fv${AUDIT_PINNED}/${name}.tgz" \
    "$sha" "${name}/cargo-audit" cargo-audit
fi

# cargo-audit clones the advisory database over the network. Without it there
# is nothing to check against, so treat it the way every other tool that may be
# absent is treated: a warning on a laptop, a failure where guards are required.
args=(audit)
[[ -n "$json" ]] && args+=(--json)

set +e
if [[ -n "$json" ]]; then
  mkdir -p "$(dirname "$json")"
  out="$(cargo-audit "${args[@]}" 2>audit.err)"
  status=$?
  printf '%s\n' "$out" >"$json"
else
  cargo-audit "${args[@]}" 2>audit.err
  status=$?
  cat audit.err >&2
fi
set -e

if [[ "$status" -ne 0 ]] && grep -qiE 'couldn.t fetch|failed to fetch|network|resolve' audit.err; then
  rm -f audit.err
  secd_missing_linter "the RustSec advisory database"
  exit 0
fi
rm -f audit.err
exit "$status"
