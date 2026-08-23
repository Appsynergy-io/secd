#!/usr/bin/env bash
# The gate. CI runs one lane per job; humans and agents run it with no
# argument, which runs every lane cheapest-first. Whatever CI checks is
# reachable from here, so nothing needs a push to GitHub to be verified.
#
#   scripts/check.sh                 every lane
#   scripts/check.sh fast            contract, shell, workflow, fmt  (~60s)
#   scripts/check.sh ui              build crates/secd-ui/dist
#   scripts/check.sh clippy test     one or more named lanes
#   scripts/check.sh pipeline --update   re-pin [pipeline] after a deliberate edit
set -euo pipefail
root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$root"
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$root/target}"
export CARGO_HOME="${CARGO_HOME:-$HOME/.cargo}"
PATH="$CARGO_HOME/bin:$HOME/.cargo/bin:$PATH"
export PATH

if [[ "${1:-}" == "pipeline" && "${2:-}" == "--update" ]]; then
  exec "$root/scripts/plan-contract.sh" --update-pipeline
fi

SECD_ROOT="$root"
SECD_TOOL_TAG="check"
export SECD_ROOT SECD_TOOL_TAG
# shellcheck source=scripts/tools.sh
. "$root/scripts/tools.sh"

ACTIONLINT_PINNED="1.7.7"

# Cheapest first: a formatting slip should not cost a full test run. `ui` is a
# prerequisite of every cargo lane, not a peer -- crates/secd-web/build.rs
# refuses to build without a fresh crates/secd-ui/dist.
ALL_LANES=(contract shell workflow fmt ui clippy test test-release compile-fail release-dry)

usage() {
  echo "usage: check.sh [lane ...]" >&2
  echo "lanes: ${ALL_LANES[*]} fast all" >&2
  echo "       pipeline --update  re-pin contract.toml [pipeline]" >&2
  exit 2
}

# ---------------------------------------------------------------- lanes

lane_contract() {
  "$root/scripts/plan-contract.sh"
}

lane_shell() {
  if ! command -v shellcheck >/dev/null 2>&1; then
    secd_missing_linter shellcheck
    return 0
  fi
  local files=()
  while IFS= read -r f; do files+=("$f"); done < <(
    git -C "$root" ls-files -- 'scripts/*.sh' 'packaging/*.sh' '.githooks/*' '.claude/hooks/*.sh'
  )
  [[ ${#files[@]} -gt 0 ]] || return 0
  shellcheck -x "${files[@]}"
}

ensure_actionlint() {
  if command -v actionlint >/dev/null 2>&1; then
    return 0
  fi
  local asset sha
  case "$(uname -s):$(uname -m)" in
    Linux:x86_64)
      asset="actionlint_${ACTIONLINT_PINNED}_linux_amd64"
      sha="023070a287cd8cccd71515fedc843f1985bf96c436b7effaecce67290e7e0757"
      ;;
    Darwin:arm64 | Darwin:aarch64)
      asset="actionlint_${ACTIONLINT_PINNED}_darwin_arm64"
      sha="2693315b9093aeacb4ebd91a993fea54fc215057bf0da2659056b4bc033873db"
      ;;
    *) return 1 ;;
  esac
  secd_fetch_tool \
    "https://github.com/rhysd/actionlint/releases/download/v${ACTIONLINT_PINNED}/${asset}.tar.gz" \
    "$sha" actionlint actionlint
}

lane_workflow() {
  if ! ensure_actionlint; then
    secd_missing_linter actionlint
    return 0
  fi
  actionlint -color
}

lane_fmt() {
  cargo fmt --all -- --check
}

# The release path used to be first executed on main, where a failure is
# already a published failure. This runs all of it against a local registry and
# a stand-in gh, for nothing.
lane_release_dry() {
  "$root/scripts/dev/release-dry.sh"
}

lane_ui() {
  "$root/scripts/build-ui.sh"
}

lane_clippy() {
  cargo clippy --locked --workspace --all-targets --no-deps -- -D warnings
}

lane_test() {
  cargo test --locked --workspace
}

lane_test_release() {
  cargo test --locked --workspace --release
}

# Secret must not implement Display/Serialize/Deref. The probe lives outside
# the workspace so it cannot take --locked; its serde pin is read from
# Cargo.lock rather than restated, which would drift silently.
lane_compile_fail() {
  local serde_ver probe status
  serde_ver="$(secd_lock_version serde)"
  [[ -n "$serde_ver" ]] || secd_die "could not read the serde version from Cargo.lock"

  probe="$(mktemp -d)"
  # shellcheck disable=SC2064
  trap "rm -rf '$probe'" RETURN
  mkdir -p "$probe/src"
  cat >"$probe/Cargo.toml" <<EOF
[package]
name = "secd-compile-fail"
version = "0.0.0"
edition = "2021"
publish = false

# mktemp follows TMPDIR. When that is under this repo (usrquota on /tmp),
# cargo otherwise treats the probe as a workspace member.
[workspace]

[dependencies]
secd-core = { path = "$root/crates/secd-core" }
serde = "=${serde_ver}"
EOF
  cp "$root/tests/compile-fail/secret_is_not_printable.rs" "$probe/src/main.rs"
  set +e
  cargo build --manifest-path "$probe/Cargo.toml" --quiet >"$probe/out" 2>&1
  status=$?
  set -e
  if [[ "$status" -eq 0 ]]; then
    echo "compile-fail: secret_is_not_printable.rs compiled; Secret must not implement Display/Serialize/Deref" >&2
    return 1
  fi
  local trait
  for trait in Display Serialize Deref; do
    if ! grep -q "$trait" "$probe/out"; then
      echo "compile-fail: expected $trait error" >&2
      cat "$probe/out" >&2
      return 1
    fi
  done
}

# ---------------------------------------------------------------- dispatch

run_lane() {
  local lane="$1"
  echo "check: ${lane}" >&2
  case "$lane" in
    contract | pipeline) lane_contract ;;
    shell) lane_shell ;;
    workflow) lane_workflow ;;
    fmt) lane_fmt ;;
    ui) lane_ui ;;
    clippy) lane_clippy ;;
    test) lane_test ;;
    test-release) lane_test_release ;;
    compile-fail) lane_compile_fail ;;
    release-dry) lane_release_dry ;;
    *)
      echo "check: unknown lane ${lane}" >&2
      usage
      ;;
  esac
}

lanes=()
if [[ $# -eq 0 ]]; then
  lanes=("${ALL_LANES[@]}")
else
  for arg in "$@"; do
    case "$arg" in
      all) lanes+=("${ALL_LANES[@]}") ;;
      fast) lanes+=(contract shell workflow fmt) ;;
      -h | --help) usage ;;
      *) lanes+=("$arg") ;;
    esac
  done
fi

for lane in "${lanes[@]}"; do
  run_lane "$lane"
done
