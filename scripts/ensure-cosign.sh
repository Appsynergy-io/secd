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

cosign_version() {
  cosign version 2>/dev/null | awk '/GitVersion/{print $2}' | cut -d+ -f1
}

# By version, not by presence -- the way scripts/audit.sh and scripts/sbom.sh
# check theirs. A cluster host with a distribution cosign on PATH used to send
# this straight to exit 0, and cosign 3 refuses a key signature that 2.5.0
# verifies ("expected key signature, not certificate"), so the deploy failed
# claiming the image was unsigned.
if [[ "$(cosign_version)" == "$COSIGN_PINNED" ]]; then
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
# Installing it is not the same as it being the one that runs: another cosign
# earlier on PATH would still win, silently, which is the bug this replaces.
if [[ "$(cosign_version)" != "$COSIGN_PINNED" ]]; then
  secd_die "cosign on PATH is $(cosign_version) not ${COSIGN_PINNED}; put ${dest} first"
fi
echo "ensure-cosign: ${dest}/cosign ${COSIGN_PINNED}" >&2
