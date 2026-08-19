#!/usr/bin/env bash
# Runs at the start of every Claude session in this repo. Deliberately small
# and non-fatal: it reports what is missing and never blocks the session.
# Nothing that compiles belongs here — scripts/check.sh installs its own
# pinned tooling on first use.
set -uo pipefail
root="${CLAUDE_PROJECT_DIR:-$(git rev-parse --show-toplevel 2>/dev/null || pwd)}"
env_file="${CLAUDE_ENV_FILE:-/dev/null}"

# An agent working on this repo must never be able to reach a real vault.
agent_home="${TMPDIR:-/tmp}/secd-agent-home"
mkdir -p "$agent_home"
printf 'export SECD_HOME=%s\n' "$agent_home" >>"$env_file"

# SECD_REQUIRE_BROWSER turns a skipped headless DOM assertion into a failure,
# so point the tests at whatever browser this machine actually has. Sandboxes
# ship chromium outside /usr/bin, which is why those assertions used to vanish.
browser=""
for candidate in \
  "${PLAYWRIGHT_BROWSERS_PATH:-/opt/pw-browsers}/chromium" \
  /opt/pw-browsers/chromium; do
  [ -x "$candidate" ] && browser="$candidate" && break
done
if [ -z "$browser" ]; then
  for name in google-chrome-stable google-chrome chromium chromium-browser; do
    browser="$(command -v "$name" 2>/dev/null)" && [ -n "$browser" ] && break
  done
fi
if [ -n "$browser" ]; then
  printf 'export SECD_BROWSER=%s\n' "$browser" >>"$env_file"
else
  echo "session-start: no browser found; scripts/check.sh test will fail under SECD_REQUIRE_BROWSER" >&2
fi

for tool in openssl git cc python3; do
  command -v "$tool" >/dev/null 2>&1 \
    || echo "session-start: ${tool} is missing; parts of the test suite need it" >&2
done
command -v keyctl >/dev/null 2>&1 \
  || echo "session-start: keyctl is missing; the DEK falls back to \$XDG_RUNTIME_DIR" >&2

# Give agents the same pre-push gate humans get.
git -C "$root" config core.hooksPath .githooks 2>/dev/null || true
exit 0
