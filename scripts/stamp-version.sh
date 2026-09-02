#!/usr/bin/env bash
# Write one version into every file that restates it.
#
# The tag history is the source of truth for a release; Cargo.toml's number is
# only the floor a maintainer can raise. The publish chain checks the tag out
# and stamps it here, so release.sh, publish-release.sh and k3s-apply.sh -- all
# of which refuse a tag that does not equal the Cargo.toml version -- become
# the proof this ran, and nothing is ever committed to main to make it true.
#
#   stamp-version.sh <X.Y.Z>
set -euo pipefail
root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$root"
SECD_ROOT="$root"
SECD_TOOL_TAG="stamp-version"
export SECD_ROOT SECD_TOOL_TAG
# shellcheck source=scripts/tools.sh
. "$root/scripts/tools.sh"

ver="${1:?stamp-version: usage: stamp-version.sh <X.Y.Z>}"
[[ "$ver" =~ ^[0-9]+\.[0-9]+\.[0-9]+([.-][0-9A-Za-z.-]+)?$ ]] \
  || secd_die "not a version: ${ver}"

old="$(
  awk '/^\[package\]/{p=1;next} /^\[/{p=0}
       p && /^version[[:space:]]*=/ {
         if (match($0, /"[^"]+"/)) { print substr($0, RSTART+1, RLENGTH-2); exit }
       }' Cargo.toml
)"
[[ -n "$old" ]] || secd_die "could not parse Cargo.toml [package] version"

# The same list plan-contract.sh rule 9 checks agree, so a file added there
# without being added here fails the contract lane rather than shipping a
# half-stamped tree.
for f in Cargo.toml crates/secd-core/Cargo.toml crates/secd-web/Cargo.toml \
  tools/import-legacy/Cargo.toml; do
  python3 - "$f" "$ver" <<'PY'
import re
import sys

path, ver = sys.argv[1], sys.argv[2]
with open(path, encoding="utf-8") as f:
    text = f.read()
head, sep, rest = text.partition("[package]")
if not sep:
    raise SystemExit(f"stamp-version: {path} has no [package]")
body, tail = rest, ""
end = re.search(r"^\[", rest, re.MULTILINE)
if end:
    body, tail = rest[: end.start()], rest[end.start() :]
body, n = re.subn(r'^version\s*=\s*"[^"]+"', f'version = "{ver}"', body, count=1, flags=re.MULTILINE)
if n != 1:
    raise SystemExit(f"stamp-version: {path} [package] has no version")
with open(path, "w", encoding="utf-8") as f:
    f.write(head + sep + body + tail)
PY
done

# One prose file, twice. plan-contract.sh compares the two byte for byte.
for f in CLAUDE.md AGENTS.md; do
  python3 - "$f" "$old" "$ver" <<'PY'
import sys

path, old, ver = sys.argv[1], sys.argv[2], sys.argv[3]
with open(path, encoding="utf-8") as f:
    text = f.read()
want = f"Version floor {old}."
if want not in text:
    raise SystemExit(f"stamp-version: {path} does not say `{want}`")
with open(path, "w", encoding="utf-8") as f:
    f.write(text.replace(want, f"Version floor {ver}.", 1))
PY
done

# The deploy manifests name the image tag the release will publish. Both may
# already carry a digest instead, which the release binds and must not lose.
python3 - "$old" "$ver" <<'PY'
import re
import sys

old, ver = sys.argv[1], sys.argv[2]
for path, pattern, repl in (
    (
        "deploy/k3s/deployment.yaml",
        r"(image:\s*\S*secd-web):" + re.escape(old) + r"\b",
        r"\g<1>:" + ver,
    ),
    (
        "deploy/k3s/kustomization.yaml",
        r'(newTag:\s*")' + re.escape(old) + r'(")',
        r"\g<1>" + ver + r"\g<2>",
    ),
):
    with open(path, encoding="utf-8") as f:
        text = f.read()
    stamped, n = re.subn(pattern, repl, text, count=1)
    if n != 1:
        if "@sha256:" in text or "digest:" in text:
            continue
        raise SystemExit(f"stamp-version: {path} names neither {old} nor a digest")
    with open(path, "w", encoding="utf-8") as f:
        f.write(stamped)
PY

# Cargo.lock restates every workspace member's version, and release.sh builds
# --locked, so a lock left behind fails the build. Rewritten in place rather
# than by `cargo update`: sign and publish deliberately never compile, and a
# release runner's registry cache is cold, where even an --offline update can
# fail. Only entries carrying a `source` are third-party; those are untouched.
python3 - "$old" "$ver" <<'PY'
import re
import sys

old, ver = sys.argv[1], sys.argv[2]
path = "Cargo.lock"
with open(path, encoding="utf-8") as f:
    text = f.read()


def stamp(block: re.Match[str]) -> str:
    body = block.group(0)
    if re.search(r"^source\s*=", body, re.MULTILINE):
        return body
    return re.sub(
        r'^version\s*=\s*"' + re.escape(old) + r'"',
        f'version = "{ver}"',
        body,
        count=1,
        flags=re.MULTILINE,
    )


stamped, n = re.subn(r"\[\[package\]\]\n(?:[^\[]|\[(?!\[))*", stamp, text)
if n == 0:
    raise SystemExit("stamp-version: Cargo.lock has no [[package]] entries")
if f'version = "{ver}"' not in stamped:
    raise SystemExit(f"stamp-version: Cargo.lock names no workspace member at {old}")
with open(path, "w", encoding="utf-8") as f:
    f.write(stamped)
PY

echo "stamp-version: ${old} -> ${ver}"
