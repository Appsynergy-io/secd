#!/usr/bin/env bash
# Pull-based deployment. Runs on the cluster host, on a timer.
#
# GitHub-hosted runners cannot reach 192.168.101.122, so the cluster fetches
# rather than being pushed to: no inbound access, and no cluster credential
# ever leaves the LAN. The release is the record of what should be running --
# the `image` job publishes image-digest.txt alongside the binaries -- and
# k3s-apply.sh refuses to apply anything whose digest does not match.
#
#   secd-agent.sh [--once] [--dry-run]
#
# Exits 0 when the cluster already runs the released digest, so a timer firing
# every few minutes is nearly free.
set -euo pipefail

root="$(cd "$(dirname "$0")/../.." && pwd)"
SECD_ROOT="$root"
SECD_TOOL_TAG="secd-agent"
export SECD_ROOT SECD_TOOL_TAG
# shellcheck source=scripts/tools.sh
. "$root/scripts/tools.sh"

dry_run=0
while [[ $# -gt 0 ]]; do
  case "$1" in
    --once) shift ;; # accepted for symmetry with the timer unit
    --dry-run)
      dry_run=1
      shift
      ;;
    *) secd_die "usage: secd-agent.sh [--once] [--dry-run]" ;;
  esac
done

REPO="${SECD_REPO:-Appsynergy-io/secd}"
DIGEST_URL="${SECD_DIGEST_URL:-https://github.com/${REPO}/releases/latest/download/image-digest.txt}"
NAMESPACE="${SECD_NAMESPACE:-secd}"
DEPLOYMENT="${SECD_DEPLOYMENT:-secd-web}"

command -v curl >/dev/null 2>&1 || secd_die "curl is required"

if [[ -n "${KUBECTL:-}" ]]; then
  kc=("$KUBECTL")
elif command -v kubectl >/dev/null 2>&1; then
  kc=(kubectl)
elif command -v k3s >/dev/null 2>&1; then
  kc=(k3s kubectl)
else
  secd_die "kubectl or k3s is required"
fi

# What the release says should be running. Same unauthenticated
# releases/latest/download path packaging/install.sh already depends on.
want="$(curl -fsSL --proto '=https' "$DIGEST_URL" 2>/dev/null | tr -d '[:space:]')" \
  || secd_die "could not fetch ${DIGEST_URL}"
[[ "$want" =~ ^sha256:[0-9a-f]{64}$ ]] \
  || secd_die "released digest is not a sha256 reference: ${want:-<empty>}"

# What is running. A tag-shaped image here means the cluster was applied by
# something other than k3s-apply.sh, which is itself worth converging.
running="$("${kc[@]}" -n "$NAMESPACE" get "deploy/${DEPLOYMENT}" \
  -o jsonpath='{.spec.template.spec.containers[0].image}' 2>/dev/null || true)"
have="${running##*@}"
[[ "$have" == "$running" ]] && have=""

if [[ "$have" == "$want" ]]; then
  echo "secd-agent: already running ${want}"
  exit 0
fi

echo "secd-agent: ${have:-<untracked>} -> ${want}"
if [[ "$dry_run" -eq 1 ]]; then
  echo "secd-agent: --dry-run, not applying"
  exit 0
fi

# Which tag that digest was published under. The pipeline derives the version
# from the tag history rather than from a committed file, so this checkout's
# Cargo.toml is a floor and names an older image -- k3s-apply.sh must be told
# the released tag, not left to read one that no longer matches.
loc="$(curl -fsSIL --proto '=https' "https://github.com/${REPO}/releases/latest" \
  | tr -d '\r' | awk 'tolower($1) == "location:" { print $2 }' | tail -n 1)"
ver="${loc##*/v}"
[[ "$ver" =~ ^[0-9]+\.[0-9]+\.[0-9]+([.-][0-9A-Za-z.-]+)?$ ]] \
  || secd_die "could not read the released version from ${loc:-<no redirect>}"

# The deploy logic is this checkout, and nothing else updates it: k3s-apply.sh,
# ensure-cosign.sh and keys/cosign.pub all run from here, so a fix to any of
# them would need a person on the host. Move to the tag the release names, and
# the release becomes the one record of what runs -- image and deploy alike.
#
# It runs the tag's code, which is the same trust as running the tag's image:
# both are what the gate and the ruleset let onto main. Detached on purpose,
# so a local edit is a refusal here rather than a merge conflict at 3am.
if [[ "${SECD_SELF_UPDATE:-1}" == "1" ]] && [[ -d "$root/.git" ]]; then
  if ! git -C "$root" diff --quiet HEAD -- 2>/dev/null; then
    secd_die "$root has uncommitted changes; refusing to move it to v${ver}"
  fi
  at="$(git -C "$root" rev-parse -q --verify HEAD 2>/dev/null || true)"
  want="$(git -C "$root" rev-parse -q --verify "refs/tags/v${ver}^{commit}" 2>/dev/null || true)"
  if [[ -z "$want" ]]; then
    git -C "$root" fetch --tags --quiet origin \
      || secd_die "could not fetch tags into $root"
    want="$(git -C "$root" rev-parse -q --verify "refs/tags/v${ver}^{commit}" 2>/dev/null || true)"
  fi
  [[ -n "$want" ]] || secd_die "v${ver} is not a tag in $root"
  if [[ "$at" != "$want" ]]; then
    echo "secd-agent: ${root} ${at:0:12} -> v${ver} (${want:0:12})"
    git -C "$root" checkout --quiet --detach "$want" \
      || secd_die "could not move $root to v${ver} -- under ProtectSystem=strict the unit needs ReadWritePaths=${root}"
    # Re-exec: every script below this line just changed underneath us.
    again=()
    [[ "$dry_run" -eq 1 ]] && again+=(--dry-run)
    exec "$root/deploy/agent/secd-agent.sh" "${again[@]}"
  fi
fi

# k3s-apply.sh re-resolves the tag, refuses a digest that does not match this
# one, verifies the image signature against keys/cosign.pub, bounds the rollout
# and rolls back on failure. The agent adds no deployment logic of its own.
exec "$root/scripts/k3s-apply.sh" \
  --image "ghcr.io/appsynergy-io/secd-web:${ver}" \
  --expect-digest "$want"
