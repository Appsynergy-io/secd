#!/usr/bin/env bash
# Put a pinned, checksum-verified cosign on PATH. Idempotent.
#
# Extracted from release.sh so the jobs that sign, and the ones that verify,
# can get cosign without invoking a script that also compiles.
set -euo pipefail
root="$(cd "$(dirname "$0")/.." && pwd)"
SECD_ROOT="$root"
SECD_TOOL_TAG="ensure-cosign"
export SECD_ROOT SECD_TOOL_TAG
# shellcheck source=scripts/tools.sh
. "$root/scripts/tools.sh"

COSIGN_PINNED="v2.5.0"

if command -v cosign >/dev/null 2>&1; then
  exit 0
fi

case "$(uname -s):$(uname -m)" in
  Linux:x86_64)
    asset="cosign-linux-amd64"
    sha="1f6c194dd0891eb345b436bb71ff9f996768355f5e0ce02dde88567029ac2188"
    ;;
  Darwin:arm64 | Darwin:aarch64)
    asset="cosign-darwin-arm64"
    sha="780da3654d9601367b0d54686ac65cb9716578610cabe292d725c7008de4db85"
    ;;
  *)
    secd_die "no pinned cosign for $(uname -s) $(uname -m)"
    ;;
esac

dest="${CARGO_HOME:-$HOME/.cargo}/bin"
mkdir -p "$dest"
tmp="$(mktemp)"
trap 'rm -f "$tmp"' EXIT
curl -fsSL --proto '=https' -o "$tmp" \
  "https://github.com/sigstore/cosign/releases/download/${COSIGN_PINNED}/${asset}" \
  || secd_die "could not download cosign ${COSIGN_PINNED}"
printf '%s  %s\n' "$sha" "$tmp" | secd_sha256 -c - >/dev/null \
  || secd_die "checksum mismatch for cosign ${COSIGN_PINNED}"
install -m 0755 "$tmp" "$dest/cosign"
echo "ensure-cosign: ${dest}/cosign" >&2
