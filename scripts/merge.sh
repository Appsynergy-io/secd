#!/usr/bin/env bash
# Run the gate, then hand the branch to GitHub to merge.
#
#   merge.sh
#
# The ruleset on main requires the `gate` check and a merge queue sits behind
# it, so the merge is GitHub's to perform. This runs the gate locally, pushes
# what it just read, and queues the pull request. The queue reruns the gate
# against the actual merge result, merges when that is green, and deletes the
# branch. Nobody comes back to press a button.
#
# Queueing is the step that was missing. `allow_auto_merge` and the queue rule
# make the repository capable of merging on its own -- `repo-settings.sh` sets
# both -- but a pull request sits open until something says go, and this script
# used to run the gate and stop. A green pull request that never merges reads
# exactly like a broken pipeline.
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
SECD_ROOT="$root"
SECD_TOOL_TAG="merge"
export SECD_ROOT SECD_TOOL_TAG
# shellcheck source=scripts/tools.sh
. "$root/scripts/tools.sh"

cd "$root"

branch="$(git rev-parse --abbrev-ref HEAD)"
[[ "$branch" != "main" ]] || secd_die "on main; branch dev-{8hex} from main first"
[[ "$branch" != "HEAD" ]] || secd_die "detached HEAD; check out a branch first"

"$root/scripts/check.sh"

command -v gh >/dev/null 2>&1 || secd_die "gh is required to queue the pull request"

# The queue must merge the commit the gate above read, not an older one.
git push --set-upstream origin "$branch"

number="$(gh pr list --head "$branch" --state open --json number --jq '.[0].number // empty')"
[[ -n "$number" ]] || secd_die "no open pull request for ${branch}; open one with gh pr create"

# --auto queues it. The merge method is the queue rule's, so naming one here is
# refused; a second run is not an error, it is the same answer.
if ! out="$(gh pr merge "$number" --auto 2>&1)"; then
  case "$out" in
    *"already queued"*) ;;
    *)
      printf '%s\n' "$out" >&2
      secd_die "could not queue pull request ${number}"
      ;;
  esac
fi

# Read the queue back rather than trusting the write: a pull request that is
# neither queued nor merged is the failure this script exists to prevent. The
# queue entry is GraphQL-only; the REST view reports auto-merge, which a queued
# pull request does not use.
slug="$(gh repo view --json nameWithOwner --jq '.nameWithOwner')"
# $owner and the rest are GraphQL variables, bound by -F below. The shell must
# leave them alone.
# shellcheck disable=SC2016
query='query($owner: String!, $name: String!, $number: Int!) {
  repository(owner: $owner, name: $name) {
    pullRequest(number: $number) { state mergeQueueEntry { state } } } }'
read -r state entry < <(
  gh api graphql \
    -F owner="${slug%%/*}" -F name="${slug##*/}" -F number="$number" \
    -f query="$query" \
    --jq '[.data.repository.pullRequest.state,
           (.data.repository.pullRequest.mergeQueueEntry.state // "-")] | join(" ")'
)

case "$state" in
  MERGED) echo "merge: pull request ${number} is merged" ;;
  OPEN)
    [[ "$entry" != "-" ]] || secd_die "pull request ${number} is open and not queued"
    echo "merge: pull request ${number} queued (${entry}); GitHub merges it when the queue's gate is green"
    ;;
  *) secd_die "pull request ${number} is ${state}" ;;
esac
