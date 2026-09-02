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
        "required_status_checks": [ { "context": "gate" } ] } },
    { "type": "merge_queue", "parameters": {
        "check_response_timeout_minutes": 60,
        "grouping_strategy": "ALLGREEN",
        "max_entries_to_build": 5,
        "max_entries_to_merge": 5,
        "merge_method": "SQUASH",
        "min_entries_to_merge": 1,
        "min_entries_to_merge_wait_minutes": 5 } }
  ]
}
JSON
)

# ci.yml has triggered on merge_group since it was written, and until this rule
# exists that trigger fires for nothing. Without a queue -- or
# strict_required_status_checks_policy, which costs a manual branch update per
# merge -- a pull request that went green against a base two merges ago still
# merges, and the gate never sees the tree that results. The queue is the
# stronger of the two: it runs the gate against the actual merge result.

# The Actions policy. sha_pinning_required is the server-side half of contract
# rule 8: the contract refuses an unpinned `uses:` in this repository's own
# files, and this refuses one at run time, including from a reusable workflow
# the contract never reads. Only GitHub's own actions and the two third-party
# ones this repo pins are allowed to run at all.
ACTIONS_POLICY='{"enabled": true, "allowed_actions": "selected", "sha_pinning_required": true}'
SELECTED_ACTIONS=$(
  cat <<'JSON'
{
  "github_owned_allowed": true,
  "verified_allowed": false,
  "patterns_allowed": ["Swatinem/rust-cache@*", "dependabot/fetch-metadata@*"]
}
JSON
)
WORKFLOW_PERMS='{"default_workflow_permissions": "read", "can_approve_pull_request_reviews": false}'

# Secret scanning, and push protection with it. This repository is a secrets
# manager; the `secrets` lane catches a value that reached a commit, and push
# protection is the half that refuses the push instead of reporting it
# afterwards. Non-provider patterns and validity checks are the two that turn
# "a string that looks like a credential" and "a credential that still works"
# into findings.
SECURITY_ANALYSIS=$(
  cat <<'JSON'
{
  "security_and_analysis": {
    "secret_scanning": { "status": "enabled" },
    "secret_scanning_push_protection": { "status": "enabled" },
    "secret_scanning_non_provider_patterns": { "status": "enabled" },
    "secret_scanning_validity_checks": { "status": "enabled" }
  }
}
JSON
)

# CodeQL default setup. `actions` scans the workflows themselves, which is the
# language most of this repository's supply-chain surface is written in.
CODEQL='{"state": "configured", "languages": ["actions", "rust"]}'

# A fork pull request runs this repository's workflows. Public repo, so this
# applies: nobody outside gets a runner without a maintainer saying so.
FORK_PR='{"approval_policy": "all_external_contributors"}'

# COSIGN_KEY lives in the `release` environment, and a workflow_dispatch can
# name any ref. Without a branch policy, a workflow on an arbitrary branch can
# reach the signing key; with it, only a v* tag can.
ENV_POLICY='{"deployment_branch_policy": {"protected_branches": false, "custom_branch_policies": true}}'
ENV_TAG_RULE='{"name": "v*", "type": "tag"}'

REPO_FLAGS='{"allow_auto_merge": true, "allow_squash_merge": true, "delete_branch_on_merge": true}'

say() { printf '%s\n' "$*"; }

if [[ "$apply" -eq 0 ]]; then
  say "repo-settings: dry run for ${REPO}. Nothing below is executed."
  say ""
  say "  POST /repos/${REPO}/rulesets"
  printf '%s\n' "$RULESET" | sed 's/^/    /'
  say ""
  say "  PUT  /repos/${REPO}/private-vulnerability-reporting"
  say ""
  say "  PUT  /repos/${REPO}/vulnerability-alerts"
  say ""
  say "  PUT  /repos/${REPO}/automated-security-fixes"
  say ""
  say "  PATCH /repos/${REPO}"
  printf '%s\n' "$REPO_FLAGS" | sed 's/^/    /'
  say ""
  say "  PUT  /repos/${REPO}/actions/permissions"
  printf '%s\n' "$ACTIONS_POLICY" | sed 's/^/    /'
  say ""
  say "  PUT  /repos/${REPO}/actions/permissions/selected-actions"
  printf '%s\n' "$SELECTED_ACTIONS" | sed 's/^/    /'
  say ""
  say "  PUT  /repos/${REPO}/actions/permissions/workflow"
  printf '%s\n' "$WORKFLOW_PERMS" | sed 's/^/    /'
  say ""
  say "  PUT  /repos/${REPO}/actions/permissions/fork-pr-contributor-approval"
  printf '%s\n' "$FORK_PR" | sed 's/^/    /'
  say ""
  say "  PATCH /repos/${REPO}   (secret scanning and push protection)"
  printf '%s\n' "$SECURITY_ANALYSIS" | sed 's/^/    /'
  say ""
  say "  PATCH /repos/${REPO}/code-scanning/default-setup"
  printf '%s\n' "$CODEQL" | sed 's/^/    /'
  say ""
  say "  PUT  /repos/${REPO}/environments/release"
  printf '%s\n' "$ENV_POLICY" | sed 's/^/    /'
  say "  POST /repos/${REPO}/environments/release/deployment-branch-policies"
  printf '%s\n' "$ENV_TAG_RULE" | sed 's/^/    /'
  say ""
  say "Re-run with --apply to perform them, then --apply --enforce once a pull"
  say "request has been seen reporting the 'gate' check."
  say ""
  say "Not done here, and not doable by API: move COSIGN_KEY and COSIGN_PASSWORD"
  say "from repository secrets into the 'release' environment. Secret values"
  say "cannot be read back, so they have to be re-entered:"
  say "  1. the environment appears by itself the first time the release runs,"
  say "     because ci.yml's sign/image jobs declare environment: release"
  say "  2. Settings > Environments > release > add both secrets, then delete"
  say "     them from Settings > Secrets and variables > Actions"
  say ""
  say "Also not doable by API: the GitHub App the ci 'dependabot' job signs in"
  say "as. A dependabot-triggered run reads Dependabot secrets, not Actions"
  say "secrets, so DEPENDABOT_APP_ID and DEPENDABOT_APP_PRIVATE_KEY go under"
  say "Settings > Secrets and variables > Dependabot. The app needs contents:"
  say "write and pull-requests: write on this repository and nothing else."
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

# The ci `deps` job runs dependency review, which reads the dependency graph.
# GitHub's own API description for this endpoint: "Enables dependency alerts and
# the dependency graph for a repository." Without it the job fails with
# "Dependency review is not supported on this repository" -- a configuration
# failure that looks exactly like a finding until you read the log. There is no
# separate endpoint for the graph alone; this is the one that turns it on.
gh api --method PUT "repos/${REPO}/vulnerability-alerts" --silent
# GET answers 204 when enabled and 404 when not, with no body to inspect.
gh api "repos/${REPO}/vulnerability-alerts" --silent >/dev/null 2>&1 \
  || secd_die "dependency alerts and the dependency graph are still disabled"
say "repo-settings: dependency alerts and the dependency graph enabled"

# Alerts say a dependency is vulnerable; this opens the pull request that fixes
# it. Without it the alert waits for someone to read it.
gh api --method PUT "repos/${REPO}/automated-security-fixes" --silent
asf="$(gh api "repos/${REPO}/automated-security-fixes" --jq '.enabled' 2>/dev/null || true)"
[[ "$asf" == "true" ]] || secd_die "automated security fixes are ${asf:-<unknown>}"
say "repo-settings: automated security fixes enabled"

# Without auto-merge the pipeline cannot merge: it is what waits for the gate
# and merges when it goes green, instead of a human coming back to press a
# button. It needs the ruleset above to have something to wait on.
# delete_branch_on_merge keeps dev-{8hex} branches from accumulating one per
# merge; squash is the merge method the queue rule names.
printf '%s' "$REPO_FLAGS" | gh api --method PATCH "repos/${REPO}" --input - --silent
flags="$(gh api "repos/${REPO}" \
  --jq '[.allow_auto_merge, .allow_squash_merge, .delete_branch_on_merge] | join(",")' 2>/dev/null || true)"
[[ "$flags" == "true,true,true" ]] \
  || secd_die "auto-merge, squash and delete-on-merge are ${flags:-<unknown>}"
say "repo-settings: auto-merge, squash merge and delete-branch-on-merge enabled"

# Which actions may run at all, and what the token they get can do. A workflow
# in this repository is pinned by contract rule 8; this is the half that holds
# for anything the contract does not read.
printf '%s' "$ACTIONS_POLICY" | gh api --method PUT "repos/${REPO}/actions/permissions" --input - --silent
printf '%s' "$SELECTED_ACTIONS" \
  | gh api --method PUT "repos/${REPO}/actions/permissions/selected-actions" --input - --silent
pol="$(gh api "repos/${REPO}/actions/permissions" \
  --jq '[.allowed_actions, (.sha_pinning_required | tostring)] | join(",")' 2>/dev/null || true)"
[[ "$pol" == "selected,true" ]] || secd_die "the Actions policy is ${pol:-<unknown>}"
say "repo-settings: Actions restricted to selected, sha pinning required"

printf '%s' "$WORKFLOW_PERMS" \
  | gh api --method PUT "repos/${REPO}/actions/permissions/workflow" --input - --silent
wf="$(gh api "repos/${REPO}/actions/permissions/workflow" \
  --jq '[.default_workflow_permissions, (.can_approve_pull_request_reviews | tostring)] | join(",")' 2>/dev/null || true)"
[[ "$wf" == "read,false" ]] || secd_die "the default workflow token is ${wf:-<unknown>}"
say "repo-settings: default GITHUB_TOKEN is read-only and cannot approve"

# Public repository, so a fork pull request would otherwise get a runner on
# arrival.
printf '%s' "$FORK_PR" \
  | gh api --method PUT "repos/${REPO}/actions/permissions/fork-pr-contributor-approval" --input - --silent
fork="$(gh api "repos/${REPO}/actions/permissions/fork-pr-contributor-approval" \
  --jq '.approval_policy' 2>/dev/null || true)"
[[ "$fork" == "all_external_contributors" ]] \
  || secd_die "fork pull request approval is ${fork:-<unknown>}"
say "repo-settings: fork pull requests need approval from a maintainer"

# Push protection is the only control here that refuses a secret rather than
# reporting one. Everything else in this repository catches a value after it
# was written down.
printf '%s' "$SECURITY_ANALYSIS" | gh api --method PATCH "repos/${REPO}" --input - --silent
scan="$(gh api "repos/${REPO}" --jq '
  .security_and_analysis
  | [ .secret_scanning.status,
      .secret_scanning_push_protection.status,
      .secret_scanning_non_provider_patterns.status,
      .secret_scanning_validity_checks.status ] | join(",")' 2>/dev/null || true)"
[[ "$scan" == "enabled,enabled,enabled,enabled" ]] \
  || secd_die "secret scanning reads ${scan:-<unknown>}, expected all four enabled"
say "repo-settings: secret scanning, push protection, non-provider patterns, validity checks"

# CodeQL default setup. The API answers 202 and runs the first scan itself.
printf '%s' "$CODEQL" \
  | gh api --method PATCH "repos/${REPO}/code-scanning/default-setup" --input - --silent
ql="$(gh api "repos/${REPO}/code-scanning/default-setup" --jq '.state' 2>/dev/null || true)"
[[ "$ql" == "configured" ]] || secd_die "CodeQL default setup is ${ql:-<unknown>}"
say "repo-settings: CodeQL default setup configured for rust and actions"

# The signing key lives in this environment. A workflow_dispatch names a ref,
# so without this any branch could reach it.
printf '%s' "$ENV_POLICY" | gh api --method PUT "repos/${REPO}/environments/release" --input - --silent
existing_policy="$(gh api "repos/${REPO}/environments/release/deployment-branch-policies" \
  --jq '[.branch_policies[].name] | join(",")' 2>/dev/null || true)"
if [[ "$existing_policy" != *'v*'* ]]; then
  printf '%s' "$ENV_TAG_RULE" \
    | gh api --method POST "repos/${REPO}/environments/release/deployment-branch-policies" \
      --input - --silent
fi
envpol="$(gh api "repos/${REPO}/environments/release/deployment-branch-policies" \
  --jq '[.branch_policies[] | select(.type == "tag") | .name] | join(",")' 2>/dev/null || true)"
[[ "$envpol" == *'v*'* ]] \
  || secd_die "the release environment allows ${envpol:-<no tag policy>}, expected v*"
say "repo-settings: the release environment is restricted to v* tags"

if [[ "$enforcement" != "active" ]]; then
  say ""
  say "repo-settings: the ruleset is in evaluate mode -- it reports but does not"
  say "block. Re-run with --apply --enforce once a pull request has been seen"
  say "reporting the 'gate' check."
fi
