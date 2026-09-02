#!/usr/bin/env bash
# Push a single static binary as a scratch image, and print the manifest
# digest on stdout.
#
# Extracted from release.sh so the job that holds `packages: write` does not
# also compile third-party code, and so a dry run can exercise this exact code
# against a local registry.
#
# The pure-python path is canonical. `docker build` stamps a creation
# timestamp and its own history, so it produces a *different digest for the
# same binary*; on a GitHub runner docker is always present, so the old
# preference for it meant CI took a path a local run never would. Set
# SECD_PUSH_WITH_DOCKER=1 to opt back in.
#
#   push-image.sh --binary PATH --image REF [--extra-tag TAG ...]
set -euo pipefail
set +o xtrace

root="$(cd "$(dirname "$0")/.." && pwd)"
SECD_ROOT="$root"
SECD_TOOL_TAG="push-image"
export SECD_ROOT SECD_TOOL_TAG
# shellcheck source=scripts/tools.sh
. "$root/scripts/tools.sh"

binary=""
image=""
extra_tags=()
while [[ $# -gt 0 ]]; do
  case "$1" in
    --binary)
      binary="${2:?push-image: --binary needs a path}"
      shift 2
      ;;
    --image)
      image="${2:?push-image: --image needs a ref}"
      shift 2
      ;;
    --extra-tag)
      extra_tags+=("${2:?push-image: --extra-tag needs a tag}")
      shift 2
      ;;
    *)
      secd_die "usage: push-image.sh --binary PATH --image REF [--extra-tag TAG ...]"
      ;;
  esac
done

[[ -n "$binary" ]] || secd_die "--binary is required"
[[ -n "$image" ]] || secd_die "--image is required"
[[ -f "$binary" ]] || secd_die "missing ${binary}"
[[ -z "${GITHUB_TOKEN:-}${GH_TOKEN:-}" ]] && secd_die "GITHUB_TOKEN or GH_TOKEN is required"

if [[ "${SECD_PUSH_WITH_DOCKER:-0}" == "1" ]]; then
  command -v docker >/dev/null 2>&1 || secd_die "SECD_PUSH_WITH_DOCKER=1 but docker is not installed"
  ctx="$(mktemp -d)"
  trap 'rm -rf "$ctx"' EXIT
  cp "$binary" "$ctx/secd-web"
  chmod 0755 "$ctx/secd-web"
  # Image expose stays off (no EXPOSE).
  cat >"$ctx/Dockerfile" <<'EOF'
FROM scratch
LABEL org.opencontainers.image.source="https://github.com/Appsynergy-io/secd"
COPY secd-web /secd-web
USER 1000
ENTRYPOINT ["/secd-web"]
EOF
  docker build -t "$image" "$ctx" >&2
  docker push "$image" >&2
  for tag in ${extra_tags[@]+"${extra_tags[@]}"}; do
    docker tag "$image" "${image%:*}:${tag}" >&2
    docker push "${image%:*}:${tag}" >&2
  done
  docker inspect --format '{{index .RepoDigests 0}}' "$image" | sed 's/.*@//'
  exit 0
fi

exec python3 - "$binary" "$image" ${extra_tags[@]+"${extra_tags[@]}"} <<'PY'
import base64
import gzip
import hashlib
import io
import json
import os
import re
import sys
import tarfile
import urllib.error
import urllib.parse
import urllib.request

bin_path, image = sys.argv[1], sys.argv[2]
extra_tags = sys.argv[3:]
token = os.environ.get("GITHUB_TOKEN") or os.environ.get("GH_TOKEN")
if not token:
    raise SystemExit("push-image: GITHUB_TOKEN or GH_TOKEN is required")
user = os.environ.get("GITHUB_ACTOR") or os.environ.get("GITHUB_USER") or "git"

host, rest = image.split("/", 1)
name, tag = rest.rsplit(":", 1)
registry = "https://" + host

# Every field that could carry a timestamp is fixed, so the same binary always
# yields the same digest. That is the property the deploy pins on.
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
    "config": {
        "Entrypoint": ["/secd-web"],
        "User": "1000",
        "Labels": {
            "org.opencontainers.image.source": "https://github.com/Appsynergy-io/secd",
        },
    },
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


def parse_challenge(www):
    realm = service = ""
    for m in re.finditer(r'([A-Za-z]+)="([^"]*)"', www):
        if m.group(1) == "realm":
            realm = m.group(2)
        elif m.group(1) == "service":
            service = m.group(2)
    return realm, service


def fetch_bearer(www):
    realm, service = parse_challenge(www)
    if not realm:
        raise RuntimeError("registry 401: missing WWW-Authenticate realm")
    if not service:
        service = host
    q = urllib.parse.urlencode(
        {"service": service, "scope": "repository:" + name + ":pull,push"}
    )
    tok_url = realm + ("&" if "?" in realm else "?") + q
    raw = base64.b64encode(f"{user}:{token}".encode()).decode()
    req = urllib.request.Request(
        tok_url, method="GET", headers={"Authorization": "Basic " + raw}
    )
    try:
        with urllib.request.urlopen(req) as resp:
            body = json.loads(resp.read().decode())
    except urllib.error.HTTPError as e:
        raise RuntimeError(f"registry token -> HTTP {e.code}") from None
    got = body.get("token") or body.get("access_token")
    if not got:
        raise RuntimeError("registry token: empty response")
    return got


def urlparse_path(url):
    return urllib.parse.urlparse(url).path


def ensure_bearer():
    # GHCR 403s a GitHub PAT sent as a registry Bearer (it does not 401),
    # so exchange at GET /v2/ before the first upload. Never send the PAT.
    global bearer
    if bearer:
        return
    req = urllib.request.Request(registry + "/v2/", method="GET")
    www = ""
    try:
        urllib.request.urlopen(req)
    except urllib.error.HTTPError as e:
        www = e.headers.get("WWW-Authenticate", "") or ""
        if not www and e.code not in (401, 403):
            raise RuntimeError(f"registry GET /v2/ -> HTTP {e.code}") from None
    if not www:
        www = f'Bearer realm="{registry}/token",service="{host}"'
    bearer = fetch_bearer(www)


def do(method, url, data=None, headers=None, retry_auth=True):
    global bearer
    ensure_bearer()
    h = {"Authorization": "Bearer " + bearer}
    if headers:
        h.update(headers)
    req = urllib.request.Request(url, data=data, method=method, headers=h)
    try:
        return urllib.request.urlopen(req)
    except urllib.error.HTTPError as e:
        www = e.headers.get("WWW-Authenticate", "") or ""
        if e.code in (401, 403) and retry_auth and www:
            bearer = fetch_bearer(www)
            return do(method, url, data=data, headers=headers, retry_auth=False)
        raise RuntimeError(f"registry {method} {urlparse_path(url)} -> HTTP {e.code}") from None


def session_url(base, loc):
    joined = urllib.parse.urljoin(base, loc)
    p = urllib.parse.urlparse(joined)
    # urljoin of a GHCR PATCH Location can collapse `uploads` to `upload`,
    # which 404s. The session from POST is `/blobs/uploads/<uuid>`.
    path = re.sub(r"/blobs/upload/(?!s)", "/blobs/uploads/", p.path)
    return urllib.parse.urlunparse(p._replace(path=path))


def put_blob(digest, blob, content_type):
    start = f"{registry}/v2/{name}/blobs/uploads/"
    with do("POST", start) as resp:
        loc = resp.headers.get("Location")
        if not loc:
            raise RuntimeError("registry upload: missing Location")
    session = session_url(start, loc)
    # GHCR 405s PATCH on the session. Monolithic PUT of the blob with
    # ?digest= on the POST session (plural /blobs/uploads/) is the path
    # that v0.1.10's docker client used.
    sep = "&" if "?" in session else "?"
    complete = session + sep + "digest=" + urllib.parse.quote(digest, safe=":")
    with do("PUT", complete, data=blob, headers={"Content-Type": content_type}):
        pass


put_blob(layer_digest, gz_bytes, "application/octet-stream")
put_blob(config_digest, config_bytes, "application/vnd.docker.container.image.v1+json")

digest = ""
for t in [tag] + extra_tags:
    man_url = f"{registry}/v2/{name}/manifests/{urllib.parse.quote(t, safe='')}"
    with do(
        "PUT",
        man_url,
        data=manifest_bytes,
        headers={"Content-Type": "application/vnd.docker.distribution.manifest.v2+json"},
    ) as resp:
        digest = (resp.headers.get("Docker-Content-Digest") or "").strip() or digest
    print(f"push-image: pushed {host}/{name}:{t}", file=sys.stderr)

if not re.fullmatch(r"sha256:[0-9a-f]{64}", digest):
    # The registry is not obliged to echo it; the manifest bytes are ours, so
    # the digest is computable either way.
    digest = "sha256:" + hashlib.sha256(manifest_bytes).hexdigest()
print(digest)
PY
