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

# secd_fetch_zip URL SHA256 MEMBER BINARY
# Download a pinned zip, verify it, install one member into CARGO_HOME/bin.
secd_fetch_zip() {
  local url="$1" want="$2" member="$3" bin="$4"
  local tmp
  tmp="$(mktemp -d)"
  # shellcheck disable=SC2064
  trap "rm -rf '$tmp'" RETURN
  curl -fsSL --proto '=https' -o "$tmp/pkg.zip" "$url" \
    || secd_die "could not download $url"
  printf '%s  %s\n' "$want" "$tmp/pkg.zip" | secd_sha256 -c - >/dev/null \
    || secd_die "checksum mismatch for $url"
  python3 -c '
import sys
import zipfile
from pathlib import Path

zf_path, dest, member = sys.argv[1], Path(sys.argv[2]).resolve(), sys.argv[3]
with zipfile.ZipFile(zf_path) as zf:
    try:
        info = zf.getinfo(member)
    except KeyError:
        sys.exit("missing " + member)
    if info.is_dir() or member.endswith("/"):
        sys.exit("missing " + member)
    target = (dest / member).resolve()
    if not target.is_relative_to(dest):
        sys.exit("zip slip")
    target.parent.mkdir(parents=True, exist_ok=True)
    target.write_bytes(zf.read(member))
' "$tmp/pkg.zip" "$tmp" "$member" || secd_die "missing $member in $url"
  mkdir -p "${CARGO_HOME:?secd_fetch_zip: CARGO_HOME required}/bin"
  install -m 0755 "$tmp/$member" "$CARGO_HOME/bin/$bin"
}

# Bun 1.4.0, sha256 per platform. Never the curl|bash installer.
BUN_PINNED="1.4.0"

secd_ensure_bun() {
  if command -v bun >/dev/null 2>&1 \
    && [[ "$(bun --version 2>/dev/null)" == "$BUN_PINNED" ]]; then
    return 0
  fi
  local asset sha
  case "$(uname -s):$(uname -m)" in
    Linux:x86_64)
      asset="bun-linux-x64"
      sha="2d03fb5fb83ac8b567aca0a281b2ce1a1a19d488f56c2968d88c3f25e92fe452"
      ;;
    Linux:aarch64 | Linux:arm64)
      asset="bun-linux-aarch64"
      sha="4b1a332ee861983eb93bcfe6f770fff94e3e31b2c388bdaea3c8ed35e58eed0e"
      ;;
    Darwin:arm64 | Darwin:aarch64)
      asset="bun-darwin-aarch64"
      sha="c669e97f6164e1c96e0701748db98dfa77492908cbd8394c7557134a735de381"
      ;;
    Darwin:x86_64)
      asset="bun-darwin-x64"
      sha="1d0211b8f1dc991182344687ad15e72ee86f154845a5f7fa477994cd341dd9b0"
      ;;
    *)
      secd_die "no pinned bun ${BUN_PINNED} for $(uname -s) $(uname -m)"
      ;;
  esac
  secd_fetch_zip \
    "https://github.com/oven-sh/bun/releases/download/bun-v${BUN_PINNED}/${asset}.zip" \
    "$sha" "${asset}/bun" bun
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
