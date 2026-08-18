#!/usr/bin/env bash
# Build signed secd for one Rust triple. Linux also builds secd-web and pushes
# ghcr.io/appsynergy-io/secd-web:${ver} (and :sha-${GITHUB_SHA} when set).
# Tag must equal v + Cargo.toml version.
set -euo pipefail
set +o xtrace

root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$root"

target="x86_64-unknown-linux-musl"
do_image=1
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
    *)
      echo "release: usage: release.sh [--target TRIPLE] [--no-image]" >&2
      exit 2
      ;;
  esac
done

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

: "${COSIGN_KEY:?release: COSIGN_KEY is required}"
: "${COSIGN_PASSWORD:?release: COSIGN_PASSWORD is required}"

ensure_cosign() {
  if command -v cosign >/dev/null 2>&1; then
    return 0
  fi
  local url sha tmp
  url="https://github.com/sigstore/cosign/releases/download/v2.5.0/cosign-linux-amd64"
  sha="1f6c194dd0891eb345b436bb71ff9f996768355f5e0ce02dde88567029ac2188"
  tmp="$(mktemp)"
  curl -fsSL -o "$tmp" "$url"
  printf '%s  %s\n' "$sha" "$tmp" | sha256sum -c -
  chmod 0755 "$tmp"
  mv "$tmp" "${TMPDIR:-/tmp}/cosign"
  PATH="${TMPDIR:-/tmp}:${PATH}"
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

push_image() {
  local bin="$1"
  local img_repo img sha_img="" last
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
  if [[ -n "${GITHUB_SHA:-}" ]]; then
    sha_img="${img_repo}:sha-${GITHUB_SHA}"
  fi
  local ctx
  ctx="$(mktemp -d)"
  cp "$bin" "$ctx/secd-web"
  chmod 0755 "$ctx/secd-web"
  # Image expose stays off (no EXPOSE).
  cat >"$ctx/Dockerfile" <<'EOF'
FROM scratch
COPY secd-web /secd-web
USER 1000
ENTRYPOINT ["/secd-web"]
EOF

  local pushed=0
  if command -v docker >/dev/null 2>&1; then
    docker build -t "$img" "$ctx"
    if [[ -n "$sha_img" ]]; then
      docker tag "$img" "$sha_img"
    fi
    if docker push "$img"; then
      if [[ -z "$sha_img" ]] || docker push "$sha_img"; then
        pushed=1
      fi
    fi
  fi
  if [[ "$pushed" -eq 0 ]] && command -v podman >/dev/null 2>&1; then
    podman build -t "$img" "$ctx"
    if [[ -n "$sha_img" ]]; then
      podman tag "$img" "$sha_img"
    fi
    if podman push "$img"; then
      if [[ -z "$sha_img" ]] || podman push "$sha_img"; then
        pushed=1
      fi
    fi
  fi
  if [[ "$pushed" -eq 0 ]]; then
    if [[ -z "${GITHUB_TOKEN:-}" && -z "${GH_TOKEN:-}" ]]; then
      echo "release: GITHUB_TOKEN or GH_TOKEN is required to push the image" >&2
      exit 1
    fi
    local extra_tags=()
    if [[ -n "$sha_img" ]]; then
      extra_tags+=("${sha_img##*:}")
    fi
    python3 - "$bin" "$img" "${extra_tags[@]}" <<'PY'
import base64
import gzip
import hashlib
import io
import json
import os
import sys
import tarfile
import urllib.error
import urllib.parse
import urllib.request

bin_path, image = sys.argv[1], sys.argv[2]
extra_tags = sys.argv[3:]
token = os.environ.get("GITHUB_TOKEN") or os.environ.get("GH_TOKEN")
if not token:
    raise SystemExit("release: GITHUB_TOKEN or GH_TOKEN is required to push the image")
user = os.environ.get("GITHUB_ACTOR") or os.environ.get("GITHUB_USER") or "git"

host, rest = image.split("/", 1)
name, tag = rest.rsplit(":", 1)
registry = "https://" + host

data = open(bin_path, "rb").read()
tar_buf = io.BytesIO()
with tarfile.open(fileobj=tar_buf, mode="w") as tf:
    info = tarfile.TarInfo(name="secd-web")
    info.size = len(data)
    info.mode = 0o755
    info.uid = 0
    info.gid = 0
    info.mtime = 0
    tf.addfile(info, io.BytesIO(data))
tar_bytes = tar_buf.getvalue()
diff_id = "sha256:" + hashlib.sha256(tar_bytes).hexdigest()
gz_bytes = gzip.compress(tar_bytes, mtime=0)
layer_digest = "sha256:" + hashlib.sha256(gz_bytes).hexdigest()

config = {
    "architecture": "amd64",
    "os": "linux",
    "config": {"Entrypoint": ["/secd-web"], "User": "1000"},
    "rootfs": {"type": "layers", "diff_ids": [diff_id]},
}
config_bytes = (json.dumps(config, separators=(",", ":"), sort_keys=True) + "\n").encode()
config_digest = "sha256:" + hashlib.sha256(config_bytes).hexdigest()

manifest = {
    "schemaVersion": 2,
    "mediaType": "application/vnd.docker.distribution.manifest.v2+json",
    "config": {
        "mediaType": "application/vnd.docker.container.image.v1+json",
        "size": len(config_bytes),
        "digest": config_digest,
    },
    "layers": [
        {
            "mediaType": "application/vnd.docker.image.rootfs.diff.tar.gzip",
            "size": len(gz_bytes),
            "digest": layer_digest,
        }
    ],
}
manifest_bytes = (json.dumps(manifest, separators=(",", ":")) + "\n").encode()

bearer = None


def auth_header():
    if bearer:
        return "Bearer " + bearer
    raw = base64.b64encode(f"{user}:{token}".encode()).decode()
    return "Basic " + raw


def do(method, url, data=None, headers=None, retry_auth=True):
    global bearer
    h = {"Authorization": auth_header()}
    if headers:
        h.update(headers)
    req = urllib.request.Request(url, data=data, method=method, headers=h)
    try:
        return urllib.request.urlopen(req)
    except urllib.error.HTTPError as e:
        if e.code == 401 and retry_auth and not bearer:
            www = e.headers.get("WWW-Authenticate", "")
            realm = service = scope = ""
            if www.startswith("Bearer "):
                for part in www[7:].split(","):
                    part = part.strip()
                    if "=" not in part:
                        continue
                    k, v = part.split("=", 1)
                    v = v.strip().strip('"')
                    if k == "realm":
                        realm = v
                    elif k == "service":
                        service = v
                    elif k == "scope":
                        scope = v
            if realm:
                q = urllib.parse.urlencode({"service": service, "scope": scope})
                tok_url = realm + ("&" if "?" in realm else "?") + q
                with do("GET", tok_url, retry_auth=False) as resp:
                    body = json.loads(resp.read().decode())
                bearer = body.get("token") or body.get("access_token")
                if bearer:
                    return do(method, url, data=data, headers=headers, retry_auth=False)
        raise RuntimeError(f"registry {method} {urlparse_path(url)} -> HTTP {e.code}") from None


def urlparse_path(url):
    return urllib.parse.urlparse(url).path


def put_blob(digest, blob, content_type):
    start = f"{registry}/v2/{name}/blobs/uploads/"
    with do("POST", start) as resp:
        loc = resp.headers.get("Location")
        if not loc:
            raise RuntimeError("registry upload: missing Location")
    upload = urllib.parse.urljoin(start, loc)
    sep = "&" if "?" in upload else "?"
    upload = upload + sep + "digest=" + urllib.parse.quote(digest, safe=":")
    with do(
        "PUT",
        upload,
        data=blob,
        headers={"Content-Type": content_type},
    ):
        pass


put_blob(layer_digest, gz_bytes, "application/octet-stream")
put_blob(config_digest, config_bytes, "application/vnd.docker.container.image.v1+json")
for t in [tag] + extra_tags:
    man_url = f"{registry}/v2/{name}/manifests/{urllib.parse.quote(t, safe='')}"
    with do(
        "PUT",
        man_url,
        data=manifest_bytes,
        headers={"Content-Type": "application/vnd.docker.distribution.manifest.v2+json"},
    ):
        pass
    print(f"release: pushed {host}/{name}:{t}", file=sys.stderr)
PY
  fi
  rm -rf "$ctx"
}

ensure_cosign
prepare_cosign_key

dist="${RELEASE_DIST:-target/release-dist}"
mkdir -p "$dist"

case "$target" in
  x86_64-unknown-linux-musl)
    if command -v x86_64-linux-musl-gcc >/dev/null 2>&1; then
      export CC_x86_64_unknown_linux_musl=x86_64-linux-musl-gcc
    elif command -v musl-gcc >/dev/null 2>&1; then
      export CC_x86_64_unknown_linux_musl=musl-gcc
    fi
    # scratch has no musl loader; default musl target is dynamically linked.
    export RUSTFLAGS="${RUSTFLAGS:+$RUSTFLAGS }-C target-feature=+crt-static"
    cargo build --release --target "$target" --bin secd
    if [[ "$do_image" -eq 1 ]]; then
      if [[ ! -d crates/secd-web ]]; then
        echo "release: crates/secd-web missing" >&2
        exit 1
      fi
      "$root/scripts/build-ui.sh"
      cargo build --release --target "$target" -p secd-web
    fi
    ;;
  aarch64-apple-darwin)
    cargo build --release --target "$target" --bin secd
    ;;
esac

out="${CARGO_TARGET_DIR:-target}/${target}/release"
src="${out}/secd"
if [[ ! -f "$src" ]]; then
  echo "release: missing ${src}" >&2
  exit 1
fi

if [[ "$target" == "x86_64-unknown-linux-musl" ]]; then
  smoke_linux "$out/secd" secd
  if [[ "$do_image" -eq 1 ]]; then
    smoke_linux "$out/secd-web" secd-web
  fi
fi

name="secd-${target}"
cp "$src" "${dist}/${name}"
chmod 0755 "${dist}/${name}"

cosign sign-blob \
  --key "$COSIGN_KEY" \
  --yes \
  --tlog-upload=false \
  --new-bundle-format=false \
  --output-signature "${dist}/${name}.sig" \
  "${dist}/${name}"

(
  cd "$dist"
  sha256sum "$name" >"SHA256SUMS-${target}"
)
if [[ "$target" == "x86_64-unknown-linux-musl" ]]; then
  cp "${dist}/SHA256SUMS-${target}" "${dist}/SHA256SUMS"
fi

digest="$(awk '{print $1; exit}' "${dist}/SHA256SUMS-${target}")"
write_latest_json "${dist}/latest.json" "$name" "$digest"
cp "${dist}/latest.json" "${dist}/latest-${target}.json"

if [[ "$do_image" -eq 1 ]]; then
  web="${out}/secd-web"
  if [[ ! -f "$web" ]]; then
    echo "release: missing ${web}" >&2
    exit 1
  fi
  push_image "$web"
fi

echo "release: ${tag} ${target} -> ${dist}"
