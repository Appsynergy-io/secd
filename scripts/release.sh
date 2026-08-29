#!/usr/bin/env bash
# Build, sign and push one Rust triple. Split into phases so the job that
# compiles third-party code never holds a secret or a write permission:
#
#   --build-only   compile + smoke test. No cosign key, no registry, no gh.
#   --sign-only    sign an existing target/release-dist. Compiles nothing.
#   --push-image   push the built secd-web. Compiles nothing.
#   (no phase)     all three, which is what a local dry run wants.
#
# --dry-run swaps destinations only -- an ephemeral cosign key, a local
# registry -- and never skips a step, or it would validate a program we do not
# ship. Tag must equal v + Cargo.toml version.
set -euo pipefail
set +o xtrace

root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$root"

target="x86_64-unknown-linux-musl"
do_image=1
do_build=0
do_sign=0
do_push=0
dry_run=0
while [[ $# -gt 0 ]]; do
  case "$1" in
    --target)
      target="${2:?release: --target needs a triple}"
      shift 2
      ;;
    --no-image)
      do_image=0
      shift
      ;;
    --build-only)
      do_build=1
      shift
      ;;
    --sign-only)
      do_sign=1
      shift
      ;;
    --push-image)
      do_push=1
      shift
      ;;
    --dry-run)
      dry_run=1
      shift
      ;;
    *)
      echo "release: usage: release.sh [--target TRIPLE] [--no-image]" >&2
      echo "                          [--build-only|--sign-only|--push-image] [--dry-run]" >&2
      exit 2
      ;;
  esac
done
if [[ $((do_build + do_sign + do_push)) -eq 0 ]]; then
  do_build=1
  do_sign=1
  do_push=1
fi

case "$target" in
  x86_64-unknown-linux-musl) ;;
  aarch64-apple-darwin) do_image=0 ;;
  *)
    echo "release: unsupported target ${target}" >&2
    exit 1
    ;;
esac

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
  echo "release: could not parse Cargo.toml [package] version" >&2
  exit 1
fi

tag="${RELEASE_TAG:-${GITHUB_REF_NAME:-}}"
if [[ -z "$tag" && "${GITHUB_REF:-}" == refs/tags/* ]]; then
  tag="${GITHUB_REF#refs/tags/}"
fi
if [[ -z "$tag" || "$tag" == "main" || "$tag" == "refs/heads/main" ]]; then
  tag="v${ver}"
fi
if [[ "$tag" != "v${ver}" ]]; then
  echo "release: tag ${tag:-<empty>} must equal v${ver}" >&2
  exit 1
fi

if [[ "$do_sign" -eq 1 && "$dry_run" -eq 0 ]]; then
  : "${COSIGN_KEY:?release: COSIGN_KEY is required}"
  : "${COSIGN_PASSWORD:?release: COSIGN_PASSWORD is required}"
fi

sha256() {
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$@"
  elif command -v shasum >/dev/null 2>&1; then
    shasum -a 256 "$@"
  else
    echo "release: sha256sum or shasum is required" >&2
    exit 1
  fi
}

ensure_cosign() {
  "$root/scripts/ensure-cosign.sh"
  PATH="${CARGO_HOME:-$HOME/.cargo}/bin:${TMPDIR:-/tmp}:${PATH}"
  export PATH
  command -v cosign >/dev/null 2>&1
}

keyfile=""
cleanup() {
  if [[ -n "$keyfile" ]]; then
    rm -f "$keyfile"
  fi
}
trap cleanup EXIT

prepare_cosign_key() {
  if [[ -f "$COSIGN_KEY" ]]; then
    return 0
  fi
  if [[ "$COSIGN_KEY" == *-----BEGIN* ]]; then
    keyfile="$(mktemp)"
    chmod 0600 "$keyfile"
    printf '%s\n' "$COSIGN_KEY" >"$keyfile"
    COSIGN_KEY="$keyfile"
    export COSIGN_KEY
    return 0
  fi
  echo "release: COSIGN_KEY is not a file and not a PEM" >&2
  exit 1
}

pkg_base="${SECD_PACKAGE_BASE:-https://github.com/Appsynergy-io/secd/releases/download/v${ver}}"
pkg_base="${pkg_base%/}"

write_latest_json() {
  local dest="$1" name="$2" digest="$3"
  local url="${pkg_base}/${name}"
  local sig="${pkg_base}/${name}.sig"
  python3 - "$dest" "$ver" "$target" "$url" "$digest" "$sig" <<'PY'
import json
import sys

dest, version, triple, url, sha256, sig = sys.argv[1:]
doc = {
    "version": version,
    "targets": {
        triple: {
            "sha256": sha256,
            "sig": sig,
            "url": url,
        }
    },
}
with open(dest, "w", encoding="utf-8") as f:
    json.dump(doc, f, indent=2, sort_keys=True)
    f.write("\n")
PY
}

smoke_linux() {
  local bin="$1" kind="$2"
  if ! command -v file >/dev/null 2>&1; then
    echo "release: file is required" >&2
    exit 1
  fi
  if ! command -v readelf >/dev/null 2>&1; then
    echo "release: readelf is required" >&2
    exit 1
  fi
  local info
  info="$(file -b "$bin")"
  if [[ "$info" != *static-pie* ]]; then
    echo "release: ${bin} is not static-pie: ${info}" >&2
    exit 1
  fi
  local headers
  headers="$(readelf -l "$bin")"
  if printf '%s\n' "$headers" | grep -q INTERP; then
    echo "release: ${bin} has INTERP" >&2
    exit 1
  fi
  case "$kind" in
    secd)
      local got
      got="$("$bin" --version)"
      if [[ "$got" != "secd ${ver}" ]]; then
        echo "release: ${bin} --version was ${got}, want secd ${ver}" >&2
        exit 1
      fi
      ;;
    secd-web)
      "$bin" --help >/dev/null
      ;;
    *)
      echo "release: unknown smoke kind ${kind}" >&2
      exit 1
      ;;
  esac
}

# Panic locations and debug info embed the paths of the machine that built the
# binary, so the same commit built in two places produced different bytes. The
# --remap-path-prefix flags below are supposed to prevent that; this is what
# proves they did. Measured baseline: a binary built without them carries 112
# absolute registry paths, and 0 with them.
no_build_paths() {
  local bin="$1" registry hits
  registry="${CARGO_HOME:-$HOME/.cargo}/registry"
  if ! command -v strings >/dev/null 2>&1; then
    echo "release: strings is required" >&2
    exit 1
  fi
  for needle in "$registry" "$root"; do
    hits="$(strings "$bin" | grep -c -F "$needle" || true)"
    if [[ "$hits" -ne 0 ]]; then
      echo "release: ${bin} embeds ${hits} build-machine paths under ${needle};" >&2
      echo "release: --remap-path-prefix is not taking effect" >&2
      strings "$bin" | grep -F "$needle" >&2 || true
      exit 1
    fi
  done
}

push_image() {
  local bin="$1"
  local img_repo img last
  if [[ -n "${SECD_IMAGE:-}" ]]; then
    last="${SECD_IMAGE##*/}"
    if [[ "$last" == *:* ]]; then
      img="$SECD_IMAGE"
      img_repo="${SECD_IMAGE%:*}"
    else
      img_repo="$SECD_IMAGE"
      img="${SECD_IMAGE}:${ver}"
    fi
  else
    img_repo="ghcr.io/appsynergy-io/secd-web"
    img="${img_repo}:${ver}"
  fi
  local args=(--binary "$bin" --image "$img")
  if [[ -n "${GITHUB_SHA:-}" ]]; then
    args+=(--extra-tag "sha-${GITHUB_SHA}")
  fi
  "$root/scripts/push-image.sh" "${args[@]}"
}

dist="${RELEASE_DIST:-target/release-dist}"
mkdir -p "$dist"

# Absolute registry and workspace paths end up in panic messages and debug
# info, so the same commit built on two machines produced different binaries.
repro_flags="--remap-path-prefix=${CARGO_HOME:-$HOME/.cargo}/registry=/cargo/registry"
repro_flags="${repro_flags} --remap-path-prefix=${root}=/src"

# A dry run swaps destinations, never steps: a throwaway key still exercises
# cosign sign-blob and the base64 signature shape src/update.rs depends on.
dry_dir=""
if [[ "$do_sign" -eq 1 ]]; then
  ensure_cosign
  if [[ "$dry_run" -eq 1 && -z "${COSIGN_KEY:-}" ]]; then
    dry_dir="$(mktemp -d)"
    trap 'rm -rf "$dry_dir"' EXIT
    export COSIGN_PASSWORD="dry-run"
    (cd "$dry_dir" && cosign generate-key-pair >/dev/null)
    export COSIGN_KEY="$dry_dir/cosign.key"
    export SECD_VERIFY_PUB="$dry_dir/cosign.pub"
  fi
  prepare_cosign_key
fi

if [[ "$do_build" -eq 1 ]]; then
if [[ -z "${CARGO_TARGET_DIR:-}" ]]; then
  # vendored openssl-sys bakes OPENSSLDIR under CARGO_TARGET_DIR; remap-path-prefix does not rewrite C strings.
  export CARGO_TARGET_DIR="${RUNNER_TEMP:-${TMPDIR:-/tmp}}/secd-release-target"
fi
case "$target" in
  x86_64-unknown-linux-musl)
    if command -v x86_64-linux-musl-gcc >/dev/null 2>&1; then
      export CC_x86_64_unknown_linux_musl=x86_64-linux-musl-gcc
    elif command -v musl-gcc >/dev/null 2>&1; then
      export CC_x86_64_unknown_linux_musl=musl-gcc
    fi
    # scratch has no musl loader; default musl target is dynamically linked.
    # Do not export RUSTFLAGS: crt-static breaks host proc-macros.
    musl_flags="${RUSTFLAGS:+$RUSTFLAGS }${repro_flags} -C target-feature=+crt-static"
    if [[ "$do_image" -eq 1 ]]; then
      if [[ ! -d crates/secd-web ]]; then
        echo "release: crates/secd-web missing" >&2
        exit 1
      fi
      "$root/scripts/build-ui.sh"
    fi
    RUSTFLAGS="$musl_flags" cargo build --locked --release --target "$target" --bin secd
    if [[ "$do_image" -eq 1 ]]; then
      RUSTFLAGS="$musl_flags" cargo build --locked --release --target "$target" -p secd-web
    fi
    ;;
  aarch64-apple-darwin)
    RUSTFLAGS="${RUSTFLAGS:+$RUSTFLAGS }${repro_flags}" \
      cargo build --locked --release --target "$target" --bin secd
    ;;
esac
fi

out="${CARGO_TARGET_DIR:-target}/${target}/release"
src="${out}/secd"
name="secd-${target}"

if [[ "$do_build" -eq 1 ]]; then
  if [[ ! -f "$src" ]]; then
    echo "release: missing ${src}" >&2
    exit 1
  fi
  if [[ "$target" == "x86_64-unknown-linux-musl" ]]; then
    smoke_linux "$out/secd" secd
    no_build_paths "$out/secd"
    if [[ "$do_image" -eq 1 ]]; then
      smoke_linux "$out/secd-web" secd-web
      no_build_paths "$out/secd-web"
      cp "$out/secd-web" "${dist}/secd-web"
      chmod 0755 "${dist}/secd-web"
    fi
  else
    no_build_paths "$src"
  fi
  cp "$src" "${dist}/${name}"
  chmod 0755 "${dist}/${name}"
fi

if [[ "$do_sign" -eq 0 ]]; then
  echo "release: ${tag} ${target} built -> ${dist}"
  exit 0
fi

if [[ ! -f "${dist}/${name}" ]]; then
  echo "release: missing ${dist}/${name}; run --build-only first" >&2
  exit 1
fi

cosign sign-blob \
  --key "$COSIGN_KEY" \
  --yes \
  --tlog-upload=false \
  --new-bundle-format=false \
  --output-signature "${dist}/${name}.sig" \
  "${dist}/${name}"

(
  cd "$dist"
  sha256 "$name" >"SHA256SUMS-${target}"
)
if [[ "$target" == "x86_64-unknown-linux-musl" ]]; then
  cp "${dist}/SHA256SUMS-${target}" "${dist}/SHA256SUMS"
fi

digest="$(awk '{print $1; exit}' "${dist}/SHA256SUMS-${target}")"
write_latest_json "${dist}/latest.json" "$name" "$digest"
cp "${dist}/latest.json" "${dist}/latest-${target}.json"

# Verify what we just signed, with the key we signed it with. Same openssl
# path src/update.rs uses, so a green run means the published signature will
# actually verify on a client.
verify_pub="${SECD_VERIFY_PUB:-$root/keys/cosign.pub}"
if command -v openssl >/dev/null 2>&1; then
  sigbin="$(mktemp)"
  base64 -d <"${dist}/${name}.sig" >"$sigbin" 2>/dev/null \
    || cp "${dist}/${name}.sig" "$sigbin"
  if ! openssl dgst -sha256 -verify "$verify_pub" \
    -signature "$sigbin" "${dist}/${name}" >/dev/null; then
    echo "release: the signature we just produced does not verify against ${verify_pub}" >&2
    rm -f "$sigbin"
    exit 1
  fi
  rm -f "$sigbin"
fi

if [[ "$do_push" -eq 1 && "$do_image" -eq 1 ]]; then
  web="${dist}/secd-web"
  [[ -f "$web" ]] || web="${out}/secd-web"
  if [[ ! -f "$web" ]]; then
    echo "release: missing secd-web; run --build-only first" >&2
    exit 1
  fi
  push_image "$web" >"${dist}/image-digest.txt"
  echo "release: image $(cat "${dist}/image-digest.txt")"
fi

echo "release: ${tag} ${target} -> ${dist}"
