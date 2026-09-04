---
name: secd
description: >
  Use a secret without ever seeing its value. Read this file first. Use whenever a task needs a token, password, API key, DSN, certificate or credential — "the command needs an API key", "set up the env for this script", "why is auth failing", "store this secret", or "/secd".
  Also use on any forge or git auth symptom, before debugging git itself: `git push`/`pull`/`clone` or `git ls-remote` failing, "could not read Username", a 401 or 403 from the forge, or `tea` reporting no login.
  Also read before writing any script, service file or CI config that references a credential.
---

# secd

You will never see a value. There is no `get`. Values are for humans, in the TUI
and the web console.

Host `secd.imabee.com:443`, AppSynergy CA only, LAN only. Home: `$SECD_HOME`,
else `$XDG_DATA_HOME/secd`, else `~/.local/share/secd`. Every read needs a live
session. Locked: tell the human to run `secd`. Message: `secd: locked — run secd`.

## Rules

1. **Run a command that needs a secret through `secd run` or `secd gitea`.** The
   child gets plaintext; you get redacted output.
2. **Write references, never values.** A provider name and a bundle name in a
   script, a unit file or CI config are safe to write and to commit.
3. **Never `cat`, `grep`, `head` or otherwise open a file that holds
   credentials.** Route it through `secd` or leave it alone.
4. **If a value ever appears in your output, stop and say so plainly.** It is
   compromised and the operator must rotate it. Do not repeat it while
   reporting.

Redaction is a net, not a wall: it masks whole values, and a value a command
re-encodes before printing (`${VAR:0:10}`, base64) passes through unmasked.

## Git and the forges

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
`secd git-credential` answers `get` only, when the parent is `git` and a bundle
serves the host git asked for. `secd gitea --install-git` wires every forge the
vault holds — gitea, github, gitlab — so a plain `git push` authenticates.

## Anything else

`secd providers` / `secd info <NAME>` then `secd run --with P=B -- CMD`.

## Commands

| Command | You may run it | About |
|---|---|---|
| `secd` | no — human only | TUI: unlock, read, add, edit |
| `secd logout` | no | Drop DEK and HTTP session |
| `secd gitea` | yes | Run a command with the gitea bundle |
| `secd git-credential` | no — git runs it | git credential helper |
| `secd run` | yes | Run a command with provider env |
| `secd ls` | yes | List secret names |
| `secd info` | yes | Show metadata for a name |
| `secd providers` | yes | List providers |
| `secd redact` | yes | Redact secret values on stdin |
| `secd gen` | yes | Generate a secret; refuses a name that exists |
| `secd doctor` | yes | Check local setup |
| `secd update` | no | Update the secd binary |

`secd update --check` reports without writing.

## Not here

No grants and no profiles. No `audit`, `versions`, `rotate`, `burn` or `restore`
subcommand. No file leases and no offline mode. An existing value is changed by
a human, in the TUI or the web console. If a task needs one of these, say so and
stop rather than looking for another spelling.
