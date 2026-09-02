#!/usr/bin/env bash
# Pin secd-web to the GHCR Docker-Content-Digest for Cargo.toml [package]
# version and apply deploy/k3s. NAD/PVC/TLS stay in nuc-k3s.
set -euo pipefail
set +o xtrace

root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$root"

image_flag=""
expect_digest=""
while [[ $# -gt 0 ]]; do
  case "$1" in
    --image)
      image_flag="${2:?k3s-apply: --image needs a ref}"
      shift 2
      ;;
    --expect-digest)
      expect_digest="${2:?k3s-apply: --expect-digest needs sha256:...}"
      shift 2
      ;;
    *)
      echo "k3s-apply: usage: k3s-apply.sh [--image REF] [--expect-digest sha256:...]" >&2
      exit 2
      ;;
  esac
done

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
  echo "k3s-apply: could not parse Cargo.toml [package] version" >&2
  exit 1
fi

IMAGE="${image_flag:-${IMAGE:-ghcr.io/appsynergy-io/secd-web:${ver}}}"
if [[ "$IMAGE" == *@* ]]; then
  echo "k3s-apply: --image must be a tag reference" >&2
  exit 1
fi
last="${IMAGE##*/}"
if [[ "$last" == *:* ]]; then
  tag="${last##*:}"
else
  tag="$IMAGE"
fi
if [[ -z "$tag" ]]; then
  echo "k3s-apply: empty image tag" >&2
  exit 1
fi

if [[ ! -d "$root/deploy/k3s" ]]; then
  echo "k3s-apply: missing deploy/k3s" >&2
  exit 1
fi

digest="$(
  python3 - "$tag" <<'PY'
import base64
import json
import os
import re
import sys
import urllib.error
import urllib.parse
import urllib.request

tag = sys.argv[1]
host = os.environ.get("SECD_REGISTRY_HOST", "ghcr.io")
name = os.environ.get("SECD_IMAGE_NAME", "appsynergy-io/secd-web")
url = f"https://{host}/v2/{name}/manifests/{urllib.parse.quote(tag, safe='')}"
accept = (
    "application/vnd.oci.image.index.v1+json, "
    "application/vnd.oci.image.manifest.v1+json, "
    "application/vnd.docker.distribution.manifest.v2+json"
)
digest_re = re.compile(r"^sha256:[0-9a-f]{64}$")


def parse_challenge(www):
    realm = service = scope = ""
    for m in re.finditer(r'([A-Za-z]+)="([^"]*)"', www):
        if m.group(1) == "realm":
            realm = m.group(2)
        elif m.group(1) == "service":
            service = m.group(2)
        elif m.group(1) == "scope":
            scope = m.group(2)
    return realm, service, scope


def fetch_bearer(www):
    realm, service, scope = parse_challenge(www)
    if not realm:
        raise SystemExit("k3s-apply: registry 401: missing WWW-Authenticate realm")
    if not service:
        service = host
    if not scope:
        scope = "repository:" + name + ":pull"
    q = urllib.parse.urlencode({"service": service, "scope": scope})
    tok_url = realm + ("&" if "?" in realm else "?") + q
    req = urllib.request.Request(tok_url, method="GET")
    pat = os.environ.get("GHCR_TOKEN") or os.environ.get("GITHUB_TOKEN") or os.environ.get("GH_TOKEN")
    if pat:
        user = os.environ.get("GITHUB_USER") or os.environ.get("GITHUB_ACTOR") or "x-access-token"
        req.add_header(
            "Authorization",
            "Basic " + base64.b64encode(f"{user}:{pat}".encode()).decode(),
        )
    try:
        with urllib.request.urlopen(req) as resp:
            body = json.loads(resp.read().decode())
    except urllib.error.HTTPError as e:
        if e.code in (403, 404):
            raise SystemExit(f"k3s-apply: GHCR has no tag {tag}") from None
        raise SystemExit(f"k3s-apply: registry token -> HTTP {e.code}") from None
    got = body.get("token") or body.get("access_token")
    if not got:
        raise SystemExit("k3s-apply: registry token: empty response")
    return got


bearer = None


def do(method, dest, retry_auth=True):
    global bearer
    headers = {"Accept": accept}
    if bearer:
        headers["Authorization"] = "Bearer " + bearer
    req = urllib.request.Request(dest, method=method, headers=headers)
    try:
        return urllib.request.urlopen(req)
    except urllib.error.HTTPError as e:
        if e.code == 401 and retry_auth and not bearer:
            bearer = fetch_bearer(e.headers.get("WWW-Authenticate", ""))
            return do(method, dest, retry_auth=False)
        if e.code in (403, 404):
            raise SystemExit(f"k3s-apply: GHCR has no tag {tag}") from None
        raise SystemExit(
            f"k3s-apply: registry {method} {urllib.parse.urlparse(dest).path} -> HTTP {e.code}"
        ) from None


try:
    with do("GET", url) as resp:
        digest = (resp.headers.get("Docker-Content-Digest") or "").strip()
except SystemExit:
    raise
except Exception:
    raise SystemExit("k3s-apply: registry lookup failed") from None

if not digest_re.fullmatch(digest):
    raise SystemExit("k3s-apply: missing or invalid Docker-Content-Digest")
print(digest)
PY
)"

if [[ ! "$digest" =~ ^sha256:[0-9a-f]{64}$ ]]; then
  echo "k3s-apply: missing or invalid Docker-Content-Digest" >&2
  exit 1
fi

# A tag is mutable. Without this, whatever GHCR currently serves for the
# version is trusted on sight -- and :0.1.10 has already resolved to three
# different digests. Pass the digest the release published.
if [[ -n "$expect_digest" && "$digest" != "$expect_digest" ]]; then
  echo "k3s-apply: ${tag} resolves to ${digest}, expected ${expect_digest}" >&2
  exit 1
fi

# The image is what actually runs, and this is the only signature check between
# GHCR and the pod. It used to skip when cosign was not on PATH -- and the
# caller most likely to be missing it is secd-agent.service, which runs on a
# timer with a minimal PATH and nobody reading its output. A guard that can
# skip itself is not a guard, so fetch the pinned cosign and fail closed.
#
# ensure-cosign.sh installs into CARGO_HOME/bin and decides whether it has
# anything to do by looking on PATH, so that directory goes on PATH first: on
# the cluster host it is not there by default, and without this the timer would
# re-download cosign on every tick.
PATH="${CARGO_HOME:-${HOME:-}/.cargo}/bin:$PATH"
export PATH
"$root/scripts/ensure-cosign.sh"
if ! command -v cosign >/dev/null 2>&1; then
  echo "k3s-apply: cosign is required to verify ${digest}" >&2
  exit 1
fi
img_host="${SECD_REGISTRY_HOST:-ghcr.io}"
img_name="${SECD_IMAGE_NAME:-appsynergy-io/secd-web}"
ghcr_pat="${GHCR_TOKEN:-${GITHUB_TOKEN:-${GH_TOKEN:-}}}"
dockercfg=""
if [[ -n "$ghcr_pat" ]]; then
  dockercfg="$(mktemp -d)"
  user="${GITHUB_USER:-${GITHUB_ACTOR:-x-access-token}}"
  python3 - "$dockercfg" "$user" "$ghcr_pat" <<'PY'
import base64, json, os, sys
cfg, user, pat = sys.argv[1], sys.argv[2], sys.argv[3]
auth = base64.b64encode(f"{user}:{pat}".encode()).decode()
os.makedirs(cfg, exist_ok=True)
with open(os.path.join(cfg, "config.json"), "w", encoding="utf-8") as f:
    json.dump({"auths": {"ghcr.io": {"auth": auth}}}, f)
os.chmod(os.path.join(cfg, "config.json"), 0o600)
PY
  export DOCKER_CONFIG="$dockercfg"
fi
# Keep cosign's own words. Sent to /dev/null, an unreadable registry and a
# cosign that cannot parse the signature both read as "is not signed", which
# is the most alarming thing this script can say and was wrong both times.
cosign_err="$(mktemp)"
if ! cosign verify --key "$root/keys/cosign.pub" --insecure-ignore-tlog \
  "${img_host}/${img_name}@${digest}" >/dev/null 2>"$cosign_err"; then
  echo "k3s-apply: cosign verify refused ${digest}:" >&2
  grep -v '^WARNING' "$cosign_err" | head -5 >&2
  rm -f "$cosign_err"
  exit 1
fi
rm -f "$cosign_err"
echo "k3s-apply: signature ok" >&2

if [[ -n "${KUBECTL:-}" ]]; then
  kc=("$KUBECTL")
elif command -v kubectl >/dev/null 2>&1; then
  kc=(kubectl)
elif command -v k3s >/dev/null 2>&1; then
  kc=(k3s kubectl)
else
  echo "k3s-apply: kubectl or k3s is required" >&2
  exit 1
fi

tmp="$(mktemp -d)"
cleanup() {
  rm -rf "$tmp" ${dockercfg:+"$dockercfg"}
}
trap cleanup EXIT

cp -a "$root/deploy/k3s/." "$tmp/"

python3 - "$tmp" "$digest" <<'PY'
import re
import sys
from pathlib import Path

tmp, digest = sys.argv[1], sys.argv[2]
if not re.fullmatch(r"sha256:[0-9a-f]{64}", digest):
    raise SystemExit("k3s-apply: missing or invalid Docker-Content-Digest")

dep = Path(tmp) / "deployment.yaml"
text = dep.read_text(encoding="utf-8")
new, n = re.subn(
    r"image:\s*ghcr\.io/appsynergy-io/secd-web:\S+",
    "image: ghcr.io/appsynergy-io/secd-web@" + digest,
    text,
    count=1,
)
if n != 1:
    raise SystemExit("k3s-apply: could not pin image in deployment.yaml")
dep.write_text(new, encoding="utf-8")

kus = Path(tmp) / "kustomization.yaml"
ktext = kus.read_text(encoding="utf-8")
knew, kn = re.subn(r'newTag:\s*"[^"]+"', "digest: " + digest, ktext, count=1)
if kn != 1:
    raise SystemExit("k3s-apply: could not pin image in kustomization.yaml")
kus.write_text(knew, encoding="utf-8")
PY

"${kc[@]}" apply -k "$tmp"
# Without a timeout a bad image blocks here forever.
if ! "${kc[@]}" -n secd rollout status deploy/secd-web --timeout=180s; then
  echo "k3s-apply: rollout failed; rolling back" >&2
  "${kc[@]}" -n secd rollout undo deploy/secd-web || true
  exit 1
fi
echo "$digest"
