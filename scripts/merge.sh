#!/usr/bin/env bash
# Gate, then merge this branch's pull request. AGENTS.md: merge only via this.
#
# The gate here is the local half; `gate` on the pull request is the half that
# is actually enforced. --auto lets GitHub merge as soon as it is green rather
# than requiring someone to come back and press the button.
set -euo pipefail
root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$root"

"$root/scripts/check.sh"

if ! command -v gh >/dev/null 2>&1; then
  echo "merge: gh is required to merge" >&2
  exit 1
fi
gh pr merge --auto --squash --delete-branch \
  || gh pr merge --squash --delete-branch
