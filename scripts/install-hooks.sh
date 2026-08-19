#!/usr/bin/env bash
# Point git at .githooks so the pre-push gate runs. Idempotent.
set -euo pipefail
root="$(cd "$(dirname "$0")/.." && pwd)"
git -C "$root" config core.hooksPath .githooks
echo "install-hooks: core.hooksPath=.githooks"
