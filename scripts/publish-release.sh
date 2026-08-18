#!/usr/bin/env bash
# Upload target/release-dist to GitHub Release v${ver}. Merge latest.json
# fragments and upload as latest.json. Requires GITHUB_TOKEN or GH_TOKEN and gh.
set -euo pipefail
set +o xtrace

root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$root"

do_files=1
do_latest=1
extra_json=()
while [[ $# -gt 0 ]]; do
  case "$1" in
    --no-latest)
      do_latest=0
      shift
      ;;
    --latest-only)
      do_files=0
      do_latest=1
      shift
      ;;
    --merge-published)
      shift
      ;;
    -*)
      echo "publish-release: usage: publish-release.sh [--no-latest|--latest-only] [extra.json ...]" >&2
      exit 2
      ;;
    *)
      extra_json+=("$1")
      shift
      ;;
  esac
done

if [[ -z "${GITHUB_TOKEN:-}" && -z "${GH_TOKEN:-}" ]]; then
  echo "publish-release: GITHUB_TOKEN or GH_TOKEN is required" >&2
  exit 1
fi
if [[ -z "${GH_TOKEN:-}" ]]; then
  GH_TOKEN="$GITHUB_TOKEN"
  export GH_TOKEN
fi

if ! command -v gh >/dev/null 2>&1; then
  echo "publish-release: gh is required" >&2
  exit 1
fi

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
if [[ ! "$ver" =~ ^[0-9]+\.[0-9]+\.[0-9]+([.-].*)?$ ]]; then
  echo "publish-release: could not parse Cargo.toml [package] version" >&2
  exit 1
fi

dist="${RELEASE_DIST:-target/release-dist}"
repo="${GITHUB_REPOSITORY:-Appsynergy-io/secd}"
rel_tag="v${ver}"

ensure_release() {
  if gh -R "$repo" release view "$rel_tag" >/dev/null 2>&1; then
    return 0
  fi
  echo "create release ${rel_tag}" >&2
  if gh -R "$repo" release create "$rel_tag" --title "secd ${ver}" --notes ""; then
    return 0
  fi
  if gh -R "$repo" release view "$rel_tag" >/dev/null 2>&1; then
    return 0
  fi
  echo "publish-release: could not create or view ${rel_tag}" >&2
  exit 1
}

upload() {
  local file="$1"
  echo "upload release ${rel_tag}: $(basename "$file")"
  gh -R "$repo" release upload "$rel_tag" "$file" --clobber
}

collect_latest_inputs() {
  local -n _out=$1
  _out=()
  if [[ -f "${dist}/latest.json" ]]; then
    _out+=("${dist}/latest.json")
  fi
  if [[ ${#extra_json[@]} -gt 0 ]]; then
    local extra
    for extra in "${extra_json[@]}"; do
      if [[ ! -f "$extra" ]]; then
        echo "publish-release: missing ${extra}" >&2
        exit 1
      fi
      _out+=("$extra")
    done
  fi
  local frag
  for frag in \
    latest-x86_64-unknown-linux-musl.json \
    latest-aarch64-apple-darwin.json; do
    if [[ -f "${dist}/${frag}" ]]; then
      _out+=("${dist}/${frag}")
    fi
  done
}

if [[ "$do_files" -eq 1 ]]; then
  if [[ ! -d "$dist" ]]; then
    echo "publish-release: missing ${dist}" >&2
    exit 1
  fi
  ensure_release
  shopt -s nullglob
  for src in "$dist"/*; do
    [[ -f "$src" ]] || continue
    base_name="$(basename "$src")"
    case "$base_name" in
      latest.json) continue ;;
    esac
    upload "$src"
  done
  shopt -u nullglob
  for src in \
    "${root}/keys/cosign.pub" \
    "${root}/packaging/install.sh"
  do
    [[ -f "$src" ]] || continue
    upload "$src"
  done
fi

if [[ "$do_latest" -eq 1 ]]; then
  inputs=()
  collect_latest_inputs inputs
  if [[ ${#inputs[@]} -eq 0 ]]; then
    echo "publish-release: no latest.json fragments to merge" >&2
    exit 1
  fi
  merged_dir="$(mktemp -d)"
  merged="${merged_dir}/latest.json"
  "$root/scripts/merge-latest-json.sh" -o "$merged" "${inputs[@]}"
  ensure_release
  upload "$merged"
  rm -rf "$merged_dir"
fi

echo "publish-release: secd/${ver} ok"
