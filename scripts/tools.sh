#!/usr/bin/env bash
# Shared helpers for the build and check scripts. Sourced, never executed.
#
# Every external tool this repo depends on is pinned to a version and a
# sha256 and installed into CARGO_HOME/bin, so a laptop, an agent sandbox and
# a CI runner all run the same bytes.

secd_die() {
  echo "${SECD_TOOL_TAG:-secd}: $*" >&2
  exit 1
}

secd_sha256() {
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$@"
  elif command -v shasum >/dev/null 2>&1; then
    shasum -a 256 "$@"
  else
    secd_die "sha256sum or shasum is required"
  fi
}

# secd_lock_version CRATE -- the version Cargo.lock resolved for CRATE.
secd_lock_version() {
  awk -v want="$1" '
    $0 == "name = \"" want "\"" { found = 1; next }
    found && /^version = / {
      gsub(/[",]/, "", $3)
      print $3
      exit
    }
  ' "${SECD_ROOT:?secd_lock_version: SECD_ROOT required}/Cargo.lock"
}

# secd_fetch_tool URL SHA256 MEMBER BINARY
# Download a pinned tarball, verify it, install one member into CARGO_HOME/bin.
secd_fetch_tool() {
  local url="$1" want="$2" member="$3" bin="$4"
  local tmp
  tmp="$(mktemp -d)"
  # shellcheck disable=SC2064
  trap "rm -rf '$tmp'" RETURN
  curl -fsSL --proto '=https' -o "$tmp/pkg.tgz" "$url" \
    || secd_die "could not download $url"
  printf '%s  %s\n' "$want" "$tmp/pkg.tgz" | secd_sha256 -c - >/dev/null \
    || secd_die "checksum mismatch for $url"
  tar -xzf "$tmp/pkg.tgz" -C "$tmp" "$member" || secd_die "missing $member in $url"
  mkdir -p "${CARGO_HOME:?secd_fetch_tool: CARGO_HOME required}/bin"
  install -m 0755 "$tmp/$member" "$CARGO_HOME/bin/$bin"
}

# secd_require_linters -- SECD_REQUIRE_LINTERS is set and not "0"/"".
secd_require_linters() {
  [[ -n "${SECD_REQUIRE_LINTERS:-}" && "${SECD_REQUIRE_LINTERS}" != "0" ]]
}

# secd_missing_linter NAME -- fail when linters are required, else warn.
secd_missing_linter() {
  if secd_require_linters; then
    secd_die "$1 is required (SECD_REQUIRE_LINTERS is set) but was not found"
  fi
  echo "${SECD_TOOL_TAG:-secd}: $1 not found — skipping (set SECD_REQUIRE_LINTERS=1 to require)" >&2
}
