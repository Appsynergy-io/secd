#!/usr/bin/env python3
"""PreToolUse guard for Bash.

Refuses the operations an agent working on this repo has no business
performing: pushing to main, surfacing a credential into a subprocess, or
publishing anything. Branch protection is the real gate; this is the cheap
local half that also applies before protection is configured.

Exit 2 blocks the call and shows stderr to the agent.
"""

import json
import re
import sys

# Match a script only where it is *invoked* -- at the start of the command, after a
# shell operator, inside $( ) or backticks, or via an interpreter. Matching the bare
# name anywhere refused `sed -n 1,5p scripts/release.sh`, which is a read, not a run,
# and a guard that blocks ordinary reading is one someone turns off.
#
# The interpreter alternatives are anchored with a lookbehind: a bare `\bsh\s+`
# also matches the trailing "sh" of "release.sh ", which refused
# `shellcheck -x scripts/release.sh scripts/k3s-apply.sh`.
#
# Backtick command substitution is deliberately not a command position here:
# markdown inline code spells script names the same way, and this repo's prose
# is full of `k3s-apply.sh`. $( ) covers substitution without that collision.
INVOKED = (
    r"(?:^|[;&|]\s*|\$\(\s*"
    r"|(?<![\w.-])(?:ba)?sh\s+|(?<![\w.-])exec\s+)"
    r"[\w./$-]*%s\.sh\b"
)

RULES = [
    (
        r"\bgit\s+push\b(?=.*(?:\bmain\b|\bHEAD:main\b))",
        "pushing to main: open a pull request instead",
    ),
    (
        r"\bgit\s+push\b(?=.*(?:--force\b|--force-with-lease\b|(?<![\w-])-f(?![\w-])))",
        "force-pushing",
    ),
    (
        r"\bgit\s+(?:push|commit)\b(?=.*--no-verify\b)",
        "bypassing the pre-push gate with --no-verify",
    ),
    (
        r"\bsecd\s+(?:run|gitea|git-credential)\b",
        "a secd command that surfaces a credential into a subprocess",
    ),
    (r"\bgh\s+release\b", "publishing a GitHub release"),
    (r"\b(?:docker|podman)\s+push\b", "pushing a container image"),
    (r"\bcosign\s+sign\b", "signing a release artifact"),
    (INVOKED % "publish-release", "publishing a GitHub release"),
    (INVOKED % "k3s-apply", "deploying to the cluster"),
]


def main() -> int:
    try:
        payload = json.load(sys.stdin)
    except (json.JSONDecodeError, ValueError):
        return 0
    if payload.get("tool_name") != "Bash":
        return 0
    command = payload.get("tool_input", {}).get("command", "")
    if not isinstance(command, str):
        return 0

    for pattern, why in RULES:
        if re.search(pattern, command):
            print(f"guard-bash: refusing {why}.", file=sys.stderr)
            return 2

    if re.search(INVOKED % "release", command) and "--dry-run" not in command:
        print(
            "guard-bash: refusing to run release.sh outside --dry-run. "
            "Use `scripts/release.sh --dry-run` to exercise the release path.",
            file=sys.stderr,
        )
        return 2
    return 0


if __name__ == "__main__":
    sys.exit(main())
