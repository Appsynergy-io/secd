#!/bin/sh
# Install secd to ~/.local/bin. Manifest sha256, then write. Fail closed.
set -eu

HOST="github.com"
MANIFEST="https://${HOST}/Appsynergy-io/secd/releases/latest/download/latest.json"
DEST="${HOME}/.local/bin/secd"

err() { printf 'secd-install: %s\n' "$*" >&2; exit 1; }

[ -n "${HOME:-}" ] || err "HOME unset"
command -v curl >/dev/null 2>&1 || err "curl is required"
command -v python3 >/dev/null 2>&1 || err "python3 is required"

sha256of() {
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$1" | awk '{print $1}'
  elif command -v shasum >/dev/null 2>&1; then
    shasum -a 256 "$1" | awk '{print $1}'
  else
    err "sha256sum is required"
  fi
}

sys="$(uname -s)"
mach="$(uname -m)"
case "${sys}:${mach}" in
  Linux:x86_64|Linux:amd64) TRIPLE="x86_64-unknown-linux-musl" ;;
  Darwin:arm64|Darwin:aarch64) TRIPLE="aarch64-apple-darwin" ;;
  *) err "unsupported target ${sys} ${mach}" ;;
esac

TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT INT TERM

curl -fsSL --proto '=https' -o "$TMP/latest.json" "$MANIFEST" \
  || err "manifest fetch failed"

eval "$(
  python3 - "$TRIPLE" "$HOST" "$TMP/latest.json" <<'PY'
import json, sys, urllib.parse
triple, host, path = sys.argv[1], sys.argv[2], sys.argv[3]
with open(path, encoding="utf-8") as f:
    doc = json.load(f)
t = doc.get("targets", {}).get(triple)
if not isinstance(t, dict):
    sys.stderr.write("secd-install: no target %s\n" % triple)
    sys.exit(1)
url = t.get("url") or ""
sha = (t.get("sha256") or "").split()[0]
p = urllib.parse.urlparse(url)
if p.scheme != "https" or p.hostname != host:
    sys.stderr.write("secd-install: refusing host\n")
    sys.exit(1)
if not sha or len(sha) != 64:
    sys.stderr.write("secd-install: bad sha256\n")
    sys.exit(1)
print("URL=%s" % json.dumps(url))
print("WANT=%s" % json.dumps(sha))
PY
)"

curl -fsSL --proto '=https' -o "$TMP/secd" "$URL" || err "download failed"
GOT="$(sha256of "$TMP/secd")"
[ "$GOT" = "$WANT" ] || err "sha256 mismatch"

mkdir -p "${HOME}/.local/bin"
chmod 0755 "$TMP/secd"
mv -f "$TMP/secd" "$DEST"
chmod 0755 "$DEST"

write_skill() {
  dir="$1"
  mkdir -p "$dir"
  cat >"$dir/SKILL.md" <<'SECDskill_EOF'
---
name: secd
description: LAN secrets store. Agents never see a value.
---

# secd

You will never see a value. There is no `get`.

Locked: tell the human to run `secd`. Message: `secd: locked — run secd`.

## Git or Gitea

Use `secd gitea -- CMD`. Never `tea login`. Never put a token in a URL.

```
secd gitea -- git push origin HEAD
secd gitea -- git pull
secd gitea -- curl -sS -H "Authorization: token $GITEA_TOKEN" "$GITEA_URL/api/v1/user"
secd gitea --install-git
```

Header is `Authorization: token …`, never Bearer.
0 bundles: exit 2, `no gitea credential — add one in secd`.
2+ bundles: exit 2, names only, `secd gitea --bundle <name> -- …`.
`secd git-credential` runs only if parent is `git` and the host matches.

## Anything else

`secd providers` / `secd info <NAME>` then `secd run --with P=B -- CMD`.

## Commands

| Command | About |
|---|---|
| `secd` | human TUI |
| `secd logout` | Drop DEK and HTTP session |
| `secd gitea` | Run a command with the gitea bundle |
| `secd git-credential` | git credential helper |
| `secd run` | Run a command with provider env |
| `secd ls` | List secret names |
| `secd info` | Show metadata for a name |
| `secd providers` | List providers |
| `secd redact` | Redact secret values on stdin |
| `secd gen` | Generate a secret |
| `secd doctor` | Check local setup |
| `secd update` | Update the secd binary |

`secd update --check` reports without writing.
SECDskill_EOF
}

write_skill "${HOME}/.claude/skills/secd"
write_skill "${HOME}/.grok/skills/secd"

printf 'secd-install: %s\n' "$DEST"
