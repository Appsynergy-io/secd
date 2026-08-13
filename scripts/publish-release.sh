#!/usr/bin/env bash
# PUT target/release-dist files to generic secd/${ver}/. latest.json is
# delete-then-put at secd/latest/latest.json; unauthenticated GET must match.
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

: "${SECD_RELEASE_TOKEN:?publish-release: SECD_RELEASE_TOKEN is required}"

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

host="${GITEA_URL:-https://git.appsynergy.io}"
owner="${GITEA_PACKAGE_OWNER:-appsynergy}"
pkg="${GITEA_PACKAGE_NAME:-secd}"
base="${host}/api/packages/${owner}/generic/${pkg}"
dist="${RELEASE_DIST:-target/release-dist}"

auth_cfg() {
  local cfg
  cfg="$(mktemp)"
  chmod 0600 "$cfg"
  printf 'header = "Authorization: token %s"\n' "$SECD_RELEASE_TOKEN" >"$cfg"
  printf '%s' "$cfg"
}

# Gitea 409s a second PUT of the same file name.
put_file() {
  local src="$1" url="$2"
  local cfg code body
  cfg="$(auth_cfg)"
  body="$(mktemp)"
  curl -sS -o /dev/null -X DELETE --config "$cfg" "$url" || true
  code="$(
    curl -sS -o "$body" -w '%{http_code}' --config "$cfg" \
      --upload-file "$src" \
      "$url"
  )"
  rm -f "$cfg"
  if [[ "$code" != "201" && "$code" != "200" && "$code" != "204" ]]; then
    echo "publish-release: PUT ${url##*/} HTTP ${code}" >&2
    rm -f "$body"
    exit 1
  fi
  rm -f "$body"
}

readback() {
  local src="$1" url="$2"
  local got
  got="$(mktemp)"
  if ! curl -fsS -o "$got" "$url"; then
    echo "publish-release: unauthenticated GET failed: ${url}" >&2
    rm -f "$got"
    exit 1
  fi
  if ! cmp -s "$src" "$got"; then
    echo "publish-release: unauthenticated read-back mismatch: ${url}" >&2
    rm -f "$got"
    exit 1
  fi
  rm -f "$got"
}

collect_latest_inputs() {
  local dest_dir="$1"
  local -n _out=$2
  local published_ver
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
  if curl -fsS -o "${dest_dir}/published.json" "${base}/latest/latest.json"; then
    published_ver="$(
      python3 - "${dest_dir}/published.json" <<'PY' 2>/dev/null || true
import json
import sys

try:
    with open(sys.argv[1], encoding="utf-8") as f:
        doc = json.load(f)
    ver = doc.get("version") if isinstance(doc, dict) else None
    if isinstance(ver, str):
        print(ver)
except Exception:
    pass
PY
    )"
    if [[ "$published_ver" == "$ver" ]]; then
      _out+=("${dest_dir}/published.json")
    fi
  fi
  local frag
  for frag in \
    latest-x86_64-unknown-linux-musl.json \
    latest-aarch64-apple-darwin.json; do
    if curl -fsS -o "${dest_dir}/${frag}" "${base}/${ver}/${frag}"; then
      _out+=("${dest_dir}/${frag}")
    fi
  done
}

if [[ "$do_files" -eq 1 ]]; then
  if [[ ! -d "$dist" ]]; then
    echo "publish-release: missing ${dist}" >&2
    exit 1
  fi
  shopt -s nullglob
  for src in "$dist"/*; do
    [[ -f "$src" ]] || continue
    base_name="$(basename "$src")"
    case "$base_name" in
      latest.json) continue ;;
    esac
    put_file "$src" "${base}/${ver}/${base_name}"
  done
  shopt -u nullglob
fi

if [[ "$do_latest" -eq 1 ]]; then
  fetch_dir="$(mktemp -d)"
  merged="$(mktemp)"
  attempts="${MERGE_RETRIES:-8}"
  sleep_s="${MERGE_SLEEP:-3}"
  ok=0
  while [[ "$attempts" -gt 0 ]]; do
    attempts=$((attempts - 1))
    inputs=()
    collect_latest_inputs "$fetch_dir" inputs
    if [[ ${#inputs[@]} -eq 0 ]]; then
      if [[ "$attempts" -eq 0 ]]; then
        echo "publish-release: no latest.json fragments to merge" >&2
        rm -rf "$fetch_dir"
        rm -f "$merged"
        exit 1
      fi
      sleep "$sleep_s"
      continue
    fi
    "$root/scripts/merge-latest-json.sh" -o "$merged" "${inputs[@]}"
    put_file "$merged" "${base}/latest/latest.json"
    readback "$merged" "${base}/latest/latest.json"
    sleep "$sleep_s"
    if curl -fsS -o "${fetch_dir}/after.json" "${base}/latest/latest.json" \
      && cmp -s "$merged" "${fetch_dir}/after.json"; then
      ok=1
      break
    fi
  done
  rm -rf "$fetch_dir"
  rm -f "$merged"
  if [[ "$ok" -ne 1 ]]; then
    echo "publish-release: latest.json did not settle" >&2
    exit 1
  fi
fi

echo "publish-release: secd/${ver} ok"
