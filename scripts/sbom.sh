#!/usr/bin/env bash
# Write a CycloneDX SBOM for the workspace to a path we choose.
#
# Installed the same way every other tool in this repo is -- pinned version,
# pinned sha256, into CARGO_HOME/bin -- rather than as a marketplace action.
# anchore/sbom-action has no output-path input, so it cannot place the file
# where publish-release.sh picks up release assets.
#
#   sbom.sh <output-path>
set -euo pipefail
root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$root"
SECD_ROOT="$root"
SECD_TOOL_TAG="sbom"
export SECD_ROOT SECD_TOOL_TAG
export CARGO_HOME="${CARGO_HOME:-$HOME/.cargo}"
PATH="$CARGO_HOME/bin:$PATH"
export PATH
# shellcheck source=scripts/tools.sh
. "$root/scripts/tools.sh"

SYFT_PINNED="1.29.0"

out="${1:?sbom: usage: sbom.sh <output-path>}"

if ! command -v syft >/dev/null 2>&1 \
  || [[ "$(syft version -o json 2>/dev/null | grep -o '"version":"[^"]*"' | head -1 | cut -d'"' -f4)" != "$SYFT_PINNED" ]]; then
  case "$(uname -s):$(uname -m)" in
    Linux:x86_64)
      asset="syft_${SYFT_PINNED}_linux_amd64"
      sha="5b01c831cb5d712899d9179cabd80f55b6708dbd36af981ce27e59b6569e6690"
      ;;
    Darwin:arm64 | Darwin:aarch64)
      asset="syft_${SYFT_PINNED}_darwin_arm64"
      sha="a91b767b2cdf1c2171c560601e640c48638e967c8124eb568fd59cc66b3adb52"
      ;;
    *) secd_die "no pinned syft for $(uname -s) $(uname -m)" ;;
  esac
  secd_fetch_tool \
    "https://github.com/anchore/syft/releases/download/v${SYFT_PINNED}/${asset}.tar.gz" \
    "$sha" syft syft
fi

mkdir -p "$(dirname "$out")"
syft scan "dir:${root}" -o cyclonedx-json="$out" -q
[[ -s "$out" ]] || secd_die "syft produced no SBOM at ${out}"
echo "sbom: ${out}"
