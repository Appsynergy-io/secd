#!/usr/bin/env bash
# The gate. CI runs one lane per job; humans and agents run it with no
# argument, which runs every lane cheapest-first. Whatever CI checks is
# reachable from here, so nothing needs a push to GitHub to be verified.
#
#   scripts/check.sh                 every lane
#   scripts/check.sh fast            contract, shell, workflow, fmt  (~60s)
#   scripts/check.sh secrets         gitleaks over the tree and the history
#   scripts/check.sh ui              build ui/dist, tsc, bun test
#   scripts/check.sh bun-audit       bun audit against ui/bun.lock
#   scripts/check.sh crypto-parity   secd-core fixture vs the bun console crypto chunk
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
ZIZMOR_PINNED="1.29.0"
GITLEAKS_PINNED="8.30.1"

# Cheapest first: a formatting slip should not cost a full test run. `ui`
# builds ui/dist; secd-web/build.rs refuses to build without a fresh one.
ALL_LANES=(contract shell workflow secrets fmt ui bun-audit crypto-parity clippy test test-release compile-fail release-dry)

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
    git -C "$root" ls-files -- 'scripts/*.sh' 'packaging/*.sh' 'deploy/agent/*.sh' '.githooks/*' '.claude/hooks/*.sh'
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

ensure_zizmor() {
  if command -v zizmor >/dev/null 2>&1; then
    return 0
  fi
  local asset sha
  case "$(uname -s):$(uname -m)" in
    Linux:x86_64)
      asset="zizmor-x86_64-unknown-linux-gnu"
      sha="dd96df044a6e8538d5f423790f453bdd03d49e5b2bcc38214acc41a2f1297839"
      ;;
    Darwin:arm64 | Darwin:aarch64)
      asset="zizmor-aarch64-apple-darwin"
      sha="720322fade9e83a9c7953944c438f2ba942636b86b96a8f0e6b15ce94c8a6b6f"
      ;;
    *) return 1 ;;
  esac
  secd_fetch_tool \
    "https://github.com/zizmorcore/zizmor/releases/download/v${ZIZMOR_PINNED}/${asset}.tar.gz" \
    "$sha" zizmor zizmor
}

# actionlint reads the schema; zizmor reads the security properties -- token
# scope, untrusted interpolation, unpinned uses, credential persistence. They
# overlap nowhere, so both run and both are required.
lane_workflow() {
  if ! ensure_actionlint; then
    secd_missing_linter actionlint
  else
    actionlint -color
  fi
  if ! ensure_zizmor; then
    secd_missing_linter zizmor
    return 0
  fi
  # --offline: the online audits want a GitHub token, and a lane that behaves
  # differently with one is a lane whose result depends on who ran it.
  local args=(--offline --no-progress)
  if [[ -n "${GITHUB_ACTIONS:-}" ]]; then
    args+=(--format=github)
  fi
  zizmor "${args[@]}" .github
}

ensure_gitleaks() {
  if command -v gitleaks >/dev/null 2>&1; then
    return 0
  fi
  local asset sha
  case "$(uname -s):$(uname -m)" in
    Linux:x86_64)
      asset="gitleaks_${GITLEAKS_PINNED}_linux_x64"
      sha="551f6fc83ea457d62a0d98237cbad105af8d557003051f41f3e7ca7b3f2470eb"
      ;;
    Darwin:arm64 | Darwin:aarch64)
      asset="gitleaks_${GITLEAKS_PINNED}_darwin_arm64"
      sha="b40ab0ae55c505963e365f271a8d3846efbc170aa17f2607f13df610a9aeb6a5"
      ;;
    *) return 1 ;;
  esac
  secd_fetch_tool \
    "https://github.com/gitleaks/gitleaks/releases/download/v${GITLEAKS_PINNED}/${asset}.tar.gz" \
    "$sha" gitleaks gitleaks
}

# This repository is a secrets manager. A value that reaches a commit is
# compromised whether or not anyone notices, so the scan covers both the tree
# as it stands and every commit that produced it -- deleting a leaked line in a
# later commit does not unpublish it. --redact keeps the finding out of the
# log: the report says which file and which rule, never the value.
#
# In CI this needs fetch-depth: 0. A shallow clone has no history to scan.
lane_secrets() {
  if ! ensure_gitleaks; then
    secd_missing_linter gitleaks
    return 0
  fi
  local args=(--config "$root/.gitleaks.toml" --redact --no-banner --exit-code 1)
  gitleaks dir . "${args[@]}"
  # Refuse rather than report a pass over one commit: a shallow clone narrows
  # this scan to nothing and says so nowhere.
  if [[ "$(git -C "$root" rev-parse --is-shallow-repository)" == "true" ]]; then
    secd_die "the history scan needs a full clone (in CI: fetch-depth: 0)"
  fi
  gitleaks git . "${args[@]}"
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

# JS+CSS in ui/dist, uncompressed. Fonts are measured at ui/dist/fonts so a
# missed copy cannot hide a 404.
UI_JS_CSS_MAX=196608
UI_FONTS_MAX=71680

ui_budgets() {
  python3 - "$root" "$UI_JS_CSS_MAX" "$UI_FONTS_MAX" <<'PY'
from pathlib import Path
import sys

root = Path(sys.argv[1])
js_css_max = int(sys.argv[2])
fonts_max = int(sys.argv[3])
dist = root / "ui" / "dist"
if not dist.is_dir():
    sys.exit("ui: missing ui/dist")
js_css = 0
for path in dist.rglob("*"):
    if path.is_file() and path.suffix.lower() in {".js", ".css"}:
        js_css += path.stat().st_size
if js_css > js_css_max:
    sys.exit(f"ui: ui/dist JS+CSS {js_css} bytes exceeds {js_css_max}")
fonts_dir = dist / "fonts"
if not fonts_dir.is_dir():
    sys.exit("ui: missing ui/dist/fonts")
fonts = 0
for path in fonts_dir.rglob("*"):
    if path.is_file():
        fonts += path.stat().st_size
if fonts > fonts_max:
    sys.exit(f"ui: ui/dist/fonts {fonts} bytes exceeds {fonts_max}")
print(f"ui: JS+CSS {js_css} bytes, fonts {fonts} bytes")
PY
}

lane_ui() {
  secd_ensure_bun
  (
    cd "$root/ui"
    bun install --frozen-lockfile --ignore-scripts
    bun x tsc --noEmit
    bun test
  )
  (
    first="$(mktemp -d)"
    second="$(mktemp -d)"
    trap 'rm -rf "$first" "$second"' EXIT
    "$root/scripts/build-ui.sh"
    cp -a "$root/ui/dist/." "$first/"
    ui_budgets
    "$root/scripts/build-ui.sh"
    cp -a "$root/ui/dist/." "$second/"
    if ! diff -rq "$first" "$second" >/dev/null; then
      diff -rq "$first" "$second" >&2 || true
      secd_die "ui/dist is not byte-identical across two builds"
    fi
  )
}

lane_bun_audit() {
  secd_ensure_bun
  (
    cd "$root/ui"
    bun install --frozen-lockfile --ignore-scripts
    bun audit
  )
}

# The crypto chunk dist/index.html loads must open the secd-core fixture.
# Missing or stale fixture is a failure; the lane does not regenerate it
# in place. dist is always rebuilt, so a leftover bundle cannot open it.
lane_crypto_parity() {
  local fixture="$root/crates/secd-core/tests/fixtures/crypto-parity.json"
  [[ -f "$fixture" ]] || secd_die "crypto-parity: missing fixture"
  mkdir -p "${TMPDIR:-$root/.tmp}"
  local generated="${TMPDIR:-$root/.tmp}/crypto-parity-generated.json"
  cargo run -p secd-core --example parity_fixture --locked --quiet >"$generated"
  if ! cmp -s "$fixture" "$generated"; then
    secd_die "crypto-parity: fixture is stale"
  fi
  "$root/scripts/build-ui.sh"
  secd_ensure_bun
  (
    cd "$root/ui"
    bun install --frozen-lockfile --ignore-scripts
    bun "$root/ui/crypto-parity.ts"
  )
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

dispatch_lane() {
  local lane="$1"
  case "$lane" in
    contract | pipeline) lane_contract ;;
    shell) lane_shell ;;
    workflow) lane_workflow ;;
    secrets) lane_secrets ;;
    fmt) lane_fmt ;;
    ui) lane_ui ;;
    bun-audit) lane_bun_audit ;;
    crypto-parity) lane_crypto_parity ;;
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

# One foldable block per lane. GitHub collapses ::group::, and a local run gets
# the same delimiters, so the output of a failing lane starts where its marker
# does instead of somewhere in the scrollback.
#
# The lane is never the operand of `||`, `&&`, `!` or an `if`: bash turns
# errexit off for the whole call chain of a function whose status is tested, so
# a lane invoked that way reports its last command's status and every command
# before it becomes advisory -- `actionlint` and `gitleaks dir` are both ahead
# of one. The closing marker comes from an EXIT trap, which fires on the path
# errexit takes out of the script.
run_lane() {
  local lane="$1"
  echo "::group::check: ${lane}" >&2
  trap 'echo "::endgroup::" >&2' EXIT
  dispatch_lane "$lane"
  trap - EXIT
  echo "::endgroup::" >&2
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
