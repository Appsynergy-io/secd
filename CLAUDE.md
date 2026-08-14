# secd

LAN-only secrets store. Humans see values in the TUI and web console; agents never see a value. There is no `secd get`.

Host: `secd.imabee.com` (`192.168.101.122`) tcp 443. AppSynergy CA only. Version 0.1.9.

## Commands

```
scripts/check.sh
scripts/plan-contract.sh
scripts/merge.sh
```

```
secd
secd logout
secd gitea [--bundle N] -- CMD
secd gitea --install-git [--bundle N]
secd git-credential
secd run [--with P=B] -- CMD
secd ls
secd info <NAME>
secd providers
secd redact
secd gen <NAME>
secd doctor
secd update
secd update --check
```

Locked: `secd: locked — run secd`. Gitea header: `Authorization: token …` (never Bearer).

## Map

| Path | Role |
|---|---|
| `src/` | CLI binary, TUI, agent commands |
| `crates/secd-core` | `Secret`, AEAD, wrap, providers |
| `crates/secd-web` | TLS 1.3 API (later) |
| `crates/secd-ui` | web console (later) |
| `contract.toml` | commands, routes, providers, test IDs, file allow-list |
| `scripts/check.sh` | rustfmt, clippy `-D warnings`, test, test --release, compile-fail, plan-contract |
| `keys/` | CA PEMs, cosign.pub (later) |
| `skills/` | grok ≡ claude (later) |

## Invariants

- Server stores ciphertext. Disk stores no vault key and no plaintext. DEK lives in the kernel keyring until `secd logout` or reboot.
- `Secret`: no `Display`/`Serialize`/`Deref`; `Debug` redacts bytes; `mlock` + `Zeroize`.
- Unlock: passkey PRF and/or password (argon2id). Terminal never prompts.
- Home: `$SECD_HOME` else `$XDG_DATA_HOME/secd` else `~/.local/share/secd`. Files: `login.session` (0600), `login.device`.
- Branch `dev-{8hex}` from `main`. Merge only via `scripts/merge.sh`.
- One prose file: this document. `CLAUDE.md` is the same bytes. README.md is the install page. No docs/ or CODE.md.
- `contract.toml` is closed. A new command, route, provider, T-ID, or `src/` file not on the allow-list fails `scripts/plan-contract.sh`.
