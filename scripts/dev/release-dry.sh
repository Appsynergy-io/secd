#!/usr/bin/env bash
# Exercise the release path end to end with no GitHub and no real key.
#
# It deliberately does not cross-compile: every defect the audit found in the
# release path was in its *logic* -- ordering, idempotence, digest stability,
# verification -- not in the compiler invocation, and a ten-minute musl build
# per run would mean nobody runs this. Synthetic binaries exercise exactly the
# code that was broken.
#
# Covered: signing and self-verification, image push determinism against a
# strict local registry, the draft -> upload -> verify -> publish ordering,
# and the three refusals that matter (no overwrite, no invented tag, no
# publish of an asset that fails verification).
set -euo pipefail

root="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$root"
SECD_ROOT="$root"
SECD_TOOL_TAG="release-dry"
export SECD_ROOT SECD_TOOL_TAG
# shellcheck source=scripts/tools.sh
. "$root/scripts/tools.sh"

command -v openssl >/dev/null 2>&1 || secd_die "openssl is required"

work="$(mktemp -d)"
registry_pid=""
cleanup() {
  [[ -n "$registry_pid" ]] && kill "$registry_pid" 2>/dev/null
  rm -rf "$work"
}
trap cleanup EXIT

ver="$(awk '/^\[package\]/{p=1;next} /^\[/{p=0} p && /^version[[:space:]]*=/{if (match($0,/"[^"]+"/)) {print substr($0,RSTART+1,RLENGTH-2); exit}}' Cargo.toml)"
tag="v${ver}"
dist="$work/dist"
mkdir -p "$dist" "$work/gh"

export COSIGN_PASSWORD="dry-run"
export GH_TOKEN="dry-run"
export GITHUB_TOKEN="dry-run"
export GITHUB_ACTOR="dry-run"
# Pin the tag rather than letting release.sh fall back to the ambient ref.
# On a pull request GITHUB_REF_NAME is "10/merge", so this lane passed on a
# laptop, where the variable is unset, and failed on every runner. A dry run
# has to describe the release it is simulating, not the event that started it.
export RELEASE_TAG="$tag"
export GITHUB_REF_NAME="$tag"
export GITHUB_REF="refs/tags/${tag}"
export SECD_GH="$root/scripts/dev/fake-gh"
export SECD_GH_DIR="$work/gh"
export RELEASE_DIST="$dist"

# ---------------------------------------------------------------- key

# Fetch the pinned cosign rather than requiring one already on PATH. A lane
# that only passes on a machine where an earlier run happened to leave a binary
# behind is not a lane; this one was green on a laptop and red on a cold runner.
"$root/scripts/ensure-cosign.sh"
PATH="${CARGO_HOME:-$HOME/.cargo}/bin:$PATH"
export PATH
command -v cosign >/dev/null 2>&1 \
  || secd_die "scripts/ensure-cosign.sh ran but cosign is still not on PATH"
(cd "$work" && cosign generate-key-pair >/dev/null)
export COSIGN_KEY="$work/cosign.key"
export SECD_VERIFY_PUB="$work/cosign.pub"

# ---------------------------------------------------------------- artifacts

for triple in x86_64-unknown-linux-musl aarch64-apple-darwin; do
  printf 'dry-run-secd-%s' "$triple" >"$dist/secd-${triple}"
done
printf 'dry-run-secd-web' >"$dist/secd-web"

for triple in x86_64-unknown-linux-musl aarch64-apple-darwin; do
  RELEASE_DIST="$dist" "$root/scripts/release.sh" \
    --target "$triple" --sign-only --dry-run --no-image >/dev/null
done
echo "release-dry: signed and self-verified both triples"

# ---------------------------------------------------------------- image

openssl req -x509 -newkey rsa:2048 -keyout "$work/k.pem" -out "$work/c.pem" \
  -days 1 -nodes -subj "/CN=127.0.0.1" -addext "subjectAltName=IP:127.0.0.1" \
  >/dev/null 2>&1
"$root/scripts/dev/fake-registry.py" --cert "$work/c.pem" --key "$work/k.pem" \
  --store "$work/registry" >"$work/port" 2>"$work/registry.log" &
registry_pid=$!
for _ in $(seq 1 50); do
  [[ -s "$work/port" ]] && break
  sleep 0.2
done
port="$(awk '{print $2; exit}' "$work/port")"
[[ -n "$port" ]] || secd_die "the local registry did not start"

export SSL_CERT_FILE="$work/c.pem"
image="127.0.0.1:${port}/appsynergy-io/secd-web:${ver}"
first="$("$root/scripts/push-image.sh" --binary "$dist/secd-web" --image "$image" 2>/dev/null)"
second="$("$root/scripts/push-image.sh" --binary "$dist/secd-web" --image "$image" 2>/dev/null)"
[[ "$first" == "$second" ]] \
  || secd_die "the image digest is not reproducible: ${first} != ${second}"
echo "release-dry: image digest reproducible (${first})"
unset SSL_CERT_FILE

# ---------------------------------------------------------------- publish

printf '%s\n' "$tag" >"$SECD_GH_DIR/tags"
"$root/scripts/publish-release.sh" --tag "$tag" --target "0000000000000000000000000000000000000000" >/dev/null
grep -q "edit ${tag} draft=False latest=True" "$SECD_GH_DIR/transitions.log" \
  || secd_die "the release never flipped out of draft"
# Everything must have been uploaded while still a draft.
if grep -q "upload ${tag} draft=False" "$SECD_GH_DIR/transitions.log"; then
  secd_die "an asset was uploaded after the release stopped being a draft"
fi
echo "release-dry: draft -> upload -> verify -> publish, in that order"

# ---------------------------------------------------------------- refusals

expect_failure() {
  local why="$1"
  shift
  if "$@" >/dev/null 2>&1; then
    secd_die "expected a refusal: ${why}"
  fi
}

expect_failure "republishing an existing release" \
  "$root/scripts/publish-release.sh" --tag "$tag" --target "0000000000000000000000000000000000000000"

fresh="$work/gh2"
mkdir -p "$fresh"
SECD_GH_DIR="$fresh" expect_failure "publishing a tag that does not exist" \
  "$root/scripts/publish-release.sh" --tag "$tag" --target "0000000000000000000000000000000000000000"

tampered="$work/gh3"
mkdir -p "$tampered"
printf '%s\n' "$tag" >"$tampered/tags"
cp -a "$dist" "$work/dist-tampered"
printf 'tampered' >>"$work/dist-tampered/secd-x86_64-unknown-linux-musl"
SECD_GH_DIR="$tampered" RELEASE_DIST="$work/dist-tampered" \
  expect_failure "publishing an asset that fails verification" \
  "$root/scripts/publish-release.sh" --tag "$tag" --target "0000000000000000000000000000000000000000"
SECD_GH_DIR="$tampered" "$SECD_GH" -R x release view "$tag" | grep -q '"isDraft": true' \
  || secd_die "a failed verification left the release visible"
echo "release-dry: refuses overwrite, invented tag, and unverified asset"
