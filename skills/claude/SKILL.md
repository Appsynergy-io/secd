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
