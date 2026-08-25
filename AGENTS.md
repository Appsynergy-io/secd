# secd

LAN-only secrets store. Humans see values in the TUI and web console; agents never see a value. There is no `secd get`.

Host: `secd.imabee.com` (`192.168.101.122`) tcp 443. AppSynergy CA only. Version 0.1.10.

Origin: `https://github.com/Appsynergy-io/secd.git`. Image: `ghcr.io/appsynergy-io/secd-web`. Apply: `scripts/k3s-apply.sh --expect-digest` against the digest the release published; `deploy/k3s` carries the version tag and the digest is bound at apply time. NAD/PVC/TLS stay in nuc-k3s.

## Commands

```
scripts/check.sh [LANE ...]
scripts/check.sh pipeline --update
scripts/plan-contract.sh
scripts/install-hooks.sh
scripts/repo-settings.sh [--apply] [--enforce]
scripts/merge.sh
scripts/k3s-apply.sh
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
| `contract.toml` | commands, routes, providers, test IDs, file allow-list |
| `scripts/check.sh` | the gate. Lanes: `contract shell workflow secrets fmt ui bun-audit crypto-parity clippy test test-release compile-fail release-dry`. No argument runs all, cheapest first; `fast` runs the four that need no cargo build |
| `scripts/tools.sh` | shared helpers: pinned tool fetch, `Cargo.lock` version lookup |
| `.gitleaks.toml` | the `secrets` lane's rule set: the defaults, plus the two paths git ignores |
| `scripts/build-ui.sh` | bun console into `ui/dist` |
| `ui/` | bun web console |
| `rust-toolchain.toml` | the toolchain, for laptop, agent sandbox and CI alike |
| `.githooks/pre-push` | fast lanes before a push; refuses `main` |
| `scripts/release.sh` | phases: `--build-only` compiles, `--sign-only` signs, `--push-image` pushes. `--dry-run` swaps destinations, never steps |
| `scripts/push-image.sh` | deterministic scratch-image push; prints the manifest digest |
| `scripts/ensure-cosign.sh` | pinned, checksummed cosign on PATH |
| `scripts/sbom.sh` | pinned syft; CycloneDX SBOM to a chosen path |
| `scripts/repo-settings.sh` | the ruleset and settings the workflows cannot set themselves; dry run by default |
| `scripts/publish-release.sh` | draft → upload → verify → publish, once |
| `scripts/dev/` | local stand-ins: strict OCI registry, `gh`, and the release dry run |
| `scripts/k3s-apply.sh` | digest-pin GHCR image and apply `deploy/k3s` |
| `deploy/k3s` | Deployment; the digest is bound at apply time, not committed |
| `deploy/agent/` | pull-based CD for the cluster host: systemd timer, no inbound access |
| `.github/workflows/ci.yml` | PR, merge queue and `main`: one lane per job behind the `gate` status; `warm` on `main` writes the shared cache |
| `.github/workflows/release.yml` | tag `v*`: preflight → build → sign → image → publish |
| `keys/cosign.pub` | verify key for `secd update` |
| `skills/` | grok ≡ claude (later) |

## Invariants

- Server stores ciphertext. Disk stores no vault key and no plaintext. DEK lives in the kernel keyring until `secd logout` or reboot.
- `PUT /api/v1/vault` replaces the whole vault, so every save goes through `policy::save_entries_read_back`: it refuses when the load dropped an entry, checks the vault against the pre-image it loaded, and reads back what it wrote. `VaultLoad.body` is that pre-image, shaped as the route takes it back.
- `Secret`: no `Display`/`Serialize`/`Deref`; `Debug` redacts bytes; `mlock` + `Zeroize`.
- Unlock: passkey PRF and/or password (argon2id). Terminal never prompts.
- Home: `$SECD_HOME` else `$XDG_DATA_HOME/secd` else `~/.local/share/secd`. Files: `login.session` (0600), `login.device`.
- Branch `dev-{8hex}` from `main`. Merge only via `scripts/merge.sh`, which runs the gate; GitHub performs the merge itself once the ruleset's required `gate` status is green. The one caller of `gh pr merge` is ci's `dependabot` job, and `--auto` arms rather than merges: GitHub still does the merging, still only on a green gate.
- Forge is GitHub. `secd update` / `install.sh` fetch `https://github.com/Appsynergy-io/secd/releases/latest/download/…`.
- Release secrets: `COSIGN_KEY`, `COSIGN_PASSWORD`, scoped to the `release` environment. Cosign is `sign-blob` on the two CLI binaries and `sign` on the image manifest, all with `--tlog-upload=false`.
- No job that compiles third-party code holds a secret or a write scope: `build.rs` from every transitive dependency runs in it. `scripts/plan-contract.sh` enforces this.
- A version is a promise about bytes. Releases are tag-triggered, built once, and published only after the draft's assets verify. Nothing uses `--clobber`.
- The release is the record of what should be deployed: the image job publishes image-digest.txt, and the apply refuses any digest that does not match it. `deploy/agent/` converges the cluster on a timer, pulling rather than being pushed to, so no cluster credential leaves the LAN.
- A released binary carries no path from the machine that built it. The release refuses one that does.
- Do not move cosign to keyless/OIDC. `src/update.rs` verifies against a pubkey compiled into the binary with no transparency-log access; keyless would need Rekor and break `secd update` on a LAN.
- DEK: kernel keyring, else `$XDG_RUNTIME_DIR/secd/` (tmpfs). `store` keeps a kernel write only if `load` reads it back.
- Sessions are rows in `secd.db`, holding a token hash and a stored deadline: a restart signs nobody out, and expiry and revocation outlive the process that set them.
- The audit chain fails closed. A write it cannot make, or a head it cannot read, fails the request being recorded; the chain never restarts from zero on an error.
- One prose file: this document. `CLAUDE.md` is the same bytes. README.md is the install page. No docs/ or CODE.md.
- `contract.toml` is closed. A new command, route, provider, T-ID, or `src/` file not on the allow-list fails `scripts/plan-contract.sh`.
- Toolchain and bun 1.4.0 are pinned by version and sha256. bun is never the curl|bash installer.
- `ui/dist` is a build input for `secd-web`. `build.rs` refuses a missing or stale one and never invokes the bundler; run `scripts/check.sh ui` first.
- A guard that can skip itself is not a guard. `SECD_REQUIRE_BROWSER=1` turns a skipped headless DOM assertion into a failure, `SECD_REQUIRE_LINTERS=1` does the same for shellcheck, actionlint, zizmor and gitleaks. CI sets both.
- The `secrets` lane is required, never advisory: gitleaks over the working tree and over every commit that produced it, redacted, and it refuses a shallow clone rather than reporting a pass over one commit. CI gives that job `fetch-depth: 0`.
- `gate` is the only check the ruleset requires, so every ci job but `warm` is one of its `needs`; `plan-contract.sh` proves it. A job outside that list can fail while the gate reports success.
- Dependabot opens minor and patch bumps grouped. ci re-pins `[pipeline]` on the bot's branch with a GitHub App token — a `GITHUB_TOKEN` push starts no run — and arms auto-merge. Anything not minor or patch stays for a human.
