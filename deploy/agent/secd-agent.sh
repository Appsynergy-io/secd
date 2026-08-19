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

# k3s-apply.sh re-resolves the tag, refuses a digest that does not match this
# one, verifies the image signature against keys/cosign.pub, bounds the rollout
# and rolls back on failure. The agent adds no deployment logic of its own.
exec "$root/scripts/k3s-apply.sh" --expect-digest "$want"
