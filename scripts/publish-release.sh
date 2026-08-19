#!/usr/bin/env bash
# Publish target/release-dist to the GitHub Release for a tag, atomically.
#
#   publish-release.sh --tag vX.Y.Z --target SHA
#
# Ordering is the whole point. The release is created as a *draft* and stays a
# draft for the entire upload window, because `releases/latest` moves to a
# release the instant it exists: publishing first meant that on every release
# there was a multi-minute window where the documented install command 404'd
# on latest.json, and a longer one where it worked on Linux and failed on
# macOS with "no target aarch64-apple-darwin". Only after every asset is
# uploaded and verified does the draft flip.
#
# --clobber is deliberately absent. On a fresh draft there is nothing to
# clobber, and its absence turns "a re-run silently replaced a published
# artifact" -- which happened three times to v0.1.10 -- into a loud error.
#
# SECD_GH points at a stand-in gh for scripts/check.sh release-dry.
set -euo pipefail
set +o xtrace

root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$root"
SECD_ROOT="$root"
SECD_TOOL_TAG="publish-release"
export SECD_ROOT SECD_TOOL_TAG
# shellcheck source=scripts/tools.sh
. "$root/scripts/tools.sh"

GH="${SECD_GH:-gh}"

tag=""
target=""
while [[ $# -gt 0 ]]; do
  case "$1" in
    --tag)
      tag="${2:?publish-release: --tag needs a tag}"
      shift 2
      ;;
    --target)
      target="${2:?publish-release: --target needs a commit sha}"
      shift 2
      ;;
    *)
      secd_die "usage: publish-release.sh --tag vX.Y.Z --target SHA"
      ;;
  esac
done

[[ -n "$tag" ]] || secd_die "--tag is required"
[[ -n "$target" ]] || secd_die "--target is required"

if [[ -z "${GH_TOKEN:-}" ]]; then
  GH_TOKEN="${GITHUB_TOKEN:?publish-release: GITHUB_TOKEN or GH_TOKEN is required}"
  export GH_TOKEN
fi
command -v "$GH" >/dev/null 2>&1 || secd_die "${GH} is required"

cargo_ver() {
  awk '
    /^\[package\]/ { p = 1; next }
    /^\[/ { p = 0 }
    p && /^version[[:space:]]*=/ {
      if (match($0, /"[^"]+"/)) {
        print substr($0, RSTART + 1, RLENGTH - 2)
        exit
      }
    }
  ' Cargo.toml
}

ver="$(cargo_ver)"
[[ "$ver" =~ ^[0-9]+\.[0-9]+\.[0-9]+([.-].*)?$ ]] \
  || secd_die "could not parse Cargo.toml [package] version"
[[ "$tag" == "v${ver}" ]] || secd_die "tag ${tag} does not match v${ver}"

dist="${RELEASE_DIST:-target/release-dist}"
[[ -d "$dist" ]] || secd_die "missing ${dist}"
repo="${GITHUB_REPOSITORY:-Appsynergy-io/secd}"
# A dry run signs with a throwaway key, so it verifies against that one.
verify_pub="${SECD_VERIFY_PUB:-$root/keys/cosign.pub}"

# ---------------------------------------------------------------- manifest

shopt -s nullglob
fragments=("$dist"/latest-*.json)
shopt -u nullglob
[[ ${#fragments[@]} -gt 0 ]] || secd_die "no latest-*.json fragments in ${dist}"
"$root/scripts/merge-latest-json.sh" -o "${dist}/latest.json" "${fragments[@]}"

# ---------------------------------------------------------------- draft

echo "publish-release: draft ${tag} at ${target}" >&2
"$GH" -R "$repo" release create "$tag" \
  --draft \
  --verify-tag \
  --target "$target" \
  --title "secd ${ver}" \
  --notes ""

uploads=()
shopt -s nullglob
for src in "$dist"/*; do
  [[ -f "$src" ]] || continue
  case "$(basename "$src")" in
    secd-web) continue ;;
  esac
  uploads+=("$src")
done
shopt -u nullglob
for src in "$root/keys/cosign.pub" "$root/packaging/install.sh"; do
  [[ -f "$src" ]] && uploads+=("$src")
done
[[ ${#uploads[@]} -gt 0 ]] || secd_die "nothing to upload from ${dist}"

echo "publish-release: uploading ${#uploads[@]} assets" >&2
"$GH" -R "$repo" release upload "$tag" "${uploads[@]}"

# ---------------------------------------------------------------- verify

# Pull the assets back through the API and check them before anyone can see
# them. Nothing verified the release it had just published; the first party to
# discover a broken one was a user running `secd update`.
check="$(mktemp -d)"
trap 'rm -rf "$check"' EXIT
"$GH" -R "$repo" release download "$tag" -D "$check"

triples="${check}/.triples"
python3 - "$check" "$ver" "$triples" <<'PY'
import json
import sys
from pathlib import Path

check, version = Path(sys.argv[1]), sys.argv[2]
manifest = json.loads((check / "latest.json").read_text(encoding="utf-8"))
if manifest.get("version") != version:
    raise SystemExit(f"publish-release: latest.json says {manifest.get('version')}, not {version}")
targets = manifest.get("targets") or {}
if not targets:
    raise SystemExit("publish-release: latest.json names no targets")
for triple, spec in targets.items():
    for key in ("url", "sha256", "sig"):
        if not spec.get(key):
            raise SystemExit(f"publish-release: {triple} has no {key}")
    asset = check / f"secd-{triple}"
    if not asset.is_file():
        raise SystemExit(f"publish-release: {asset.name} was not uploaded")
print("publish-release: manifest names " + ", ".join(sorted(targets)), file=sys.stderr)
Path(sys.argv[3]).write_text("\n".join(sorted(targets)) + "\n")
PY

# Verify exactly what latest.json promises clients, not whatever a glob finds:
# secd-web is an input to the image push, carries no signature, and is not a
# release asset.
while IFS= read -r triple; do
  [[ -n "$triple" ]] || continue
  asset="${check}/secd-${triple}"
  name="secd-${triple}"
  want="$(awk -v n="$name" '$2 == n || $2 == "*" n {print $1; exit}' \
    "${check}/SHA256SUMS-${name#secd-}" 2>/dev/null || true)"
  got="$(secd_sha256 "$asset" | awk '{print $1}')"
  [[ -n "$want" && "$want" == "$got" ]] \
    || secd_die "${name}: published bytes do not match SHA256SUMS"
  [[ -f "${asset}.sig" ]] || secd_die "${name}: no signature was published"
  if command -v openssl >/dev/null 2>&1; then
    sigbin="${check}/${name}.sig.bin"
    base64 -d <"${asset}.sig" >"$sigbin" 2>/dev/null || cp "${asset}.sig" "$sigbin"
    openssl dgst -sha256 -verify "$verify_pub" \
      -signature "$sigbin" "$asset" >/dev/null \
      || secd_die "${name}: published signature does not verify against ${verify_pub}"
  fi
  echo "publish-release: verified ${name}" >&2
done <"$triples"

# ---------------------------------------------------------------- publish

"$GH" -R "$repo" release edit "$tag" --draft=false --latest
echo "publish-release: ${tag} published"
