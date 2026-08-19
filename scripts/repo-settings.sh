#!/usr/bin/env bash
# Apply the repository settings the workflows depend on but cannot set
# themselves. Prints every call by default; --apply performs them and re-reads
# each resource to confirm the write landed.
#
#   repo-settings.sh [--apply] [--enforce] [--repo OWNER/NAME]
#
# Without a ruleset on main, everything the pipeline enforces is advisory: a red
# pull request can still merge and a direct push still lands. This is the one
# gap the repo cannot close from inside itself.
#
# --enforce sets the ruleset to "active". The default is "evaluate", a mode that
# reports against real pull requests without blocking them, so the ruleset is
# proved before it can lock anyone out. Run once without, watch a pull request
# report the `gate` check, then run again with --enforce.
#
# The payload below follows GitHub's own OpenAPI description: `name` and
# `enforcement` are required; enforcement is disabled|active|evaluate; each
# required status check needs a `context`; the pull_request rule requires all
# five of its parameters.
set -euo pipefail
set +o xtrace

root="$(cd "$(dirname "$0")/.." && pwd)"
SECD_ROOT="$root"
SECD_TOOL_TAG="repo-settings"
export SECD_ROOT SECD_TOOL_TAG
# shellcheck source=scripts/tools.sh
. "$root/scripts/tools.sh"

REPO="${SECD_REPO:-Appsynergy-io/secd}"
apply=0
enforcement="evaluate"
while [[ $# -gt 0 ]]; do
  case "$1" in
    --apply)
      apply=1
      shift
      ;;
    --enforce)
      enforcement="active"
      shift
      ;;
    --repo)
      REPO="${2:?repo-settings: --repo needs OWNER/NAME}"
      shift 2
      ;;
    *) secd_die "usage: repo-settings.sh [--apply] [--enforce] [--repo OWNER/NAME]" ;;
  esac
done

# Refuse before any write if this is not the repository we are checked out in.
origin="$(git -C "$root" remote get-url origin 2>/dev/null || true)"
origin="${origin%.git}"
if [[ -n "$origin" && "${origin,,}" != *"${REPO,,}" ]]; then
  secd_die "refusing: --repo ${REPO} does not match origin ${origin}"
fi

RULESET=$(
  cat <<JSON
{
  "name": "main",
  "target": "branch",
  "enforcement": "${enforcement}",
  "conditions": { "ref_name": { "include": ["refs/heads/main"], "exclude": [] } },
  "rules": [
    { "type": "deletion" },
    { "type": "non_fast_forward" },
    { "type": "required_linear_history" },
    { "type": "pull_request", "parameters": {
        "dismiss_stale_reviews_on_push": false,
        "require_code_owner_review": false,
        "require_last_push_approval": false,
        "required_approving_review_count": 0,
        "required_review_thread_resolution": false } },
    { "type": "required_status_checks", "parameters": {
        "do_not_enforce_on_create": false,
        "strict_required_status_checks_policy": false,
        "required_status_checks": [ { "context": "gate" } ] } }
  ]
}
JSON
)

say() { printf '%s\n' "$*"; }

if [[ "$apply" -eq 0 ]]; then
  say "repo-settings: dry run for ${REPO}. Nothing below is executed."
  say ""
  say "  POST /repos/${REPO}/rulesets"
  printf '%s\n' "$RULESET" | sed 's/^/    /'
  say ""
  say "  PUT  /repos/${REPO}/private-vulnerability-reporting"
  say ""
  say "Re-run with --apply to perform them, then --apply --enforce once a pull"
  say "request has been seen reporting the 'gate' check."
  say ""
  say "Not done here, and not doable by API: move COSIGN_KEY and COSIGN_PASSWORD"
  say "from repository secrets into the 'release' environment. Secret values"
  say "cannot be read back, so they have to be re-entered:"
  say "  1. the environment appears by itself the first time the release runs,"
  say "     because release.yml declares environment: release"
  say "  2. Settings > Environments > release > add both secrets, then delete"
  say "     them from Settings > Secrets and variables > Actions"
  exit 0
fi

# Only the apply path needs gh; a dry run must be inspectable anywhere.
command -v gh >/dev/null 2>&1 || secd_die "gh is required to apply"

say "repo-settings: applying to ${REPO}"

existing="$(gh api "repos/${REPO}/rulesets" --jq '.[] | select(.name == "main") | .id' 2>/dev/null || true)"
if [[ -n "$existing" ]]; then
  say "repo-settings: updating ruleset ${existing}"
  printf '%s' "$RULESET" | gh api --method PUT "repos/${REPO}/rulesets/${existing}" --input - >/dev/null
else
  printf '%s' "$RULESET" | gh api --method POST "repos/${REPO}/rulesets" --input - >/dev/null
fi

# Read it back rather than trusting the write.
got="$(gh api "repos/${REPO}/rulesets" --jq '.[] | select(.name == "main") | .enforcement' 2>/dev/null || true)"
[[ "$got" == "$enforcement" ]] \
  || secd_die "ruleset enforcement is ${got:-<absent>}, expected ${enforcement}"
id="$(gh api "repos/${REPO}/rulesets" --jq '.[] | select(.name == "main") | .id')"
ctx="$(gh api "repos/${REPO}/rulesets/${id}" \
  --jq '[.rules[] | select(.type == "required_status_checks")
         | .parameters.required_status_checks[].context] | join(",")' 2>/dev/null || true)"
[[ "$ctx" == *gate* ]] || secd_die "the ruleset does not require the gate check (got: ${ctx:-none})"
say "repo-settings: ruleset ${id} enforcement=${got} requires=${ctx}"

gh api --method PUT "repos/${REPO}/private-vulnerability-reporting" --silent
pvr="$(gh api "repos/${REPO}/private-vulnerability-reporting" --jq '.enabled' 2>/dev/null || true)"
[[ "$pvr" == "true" ]] || secd_die "private vulnerability reporting is ${pvr:-<unknown>}"
say "repo-settings: private vulnerability reporting enabled"

if [[ "$enforcement" != "active" ]]; then
  say ""
  say "repo-settings: the ruleset is in evaluate mode -- it reports but does not"
  say "block. Re-run with --apply --enforce once a pull request has been seen"
  say "reporting the 'gate' check."
fi
