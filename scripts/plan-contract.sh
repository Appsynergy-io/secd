#!/usr/bin/env bash
set -euo pipefail
root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$root"
exec python3 - "$root" "$@" <<'PY'
import hashlib
import re
import subprocess
import sys
import tomllib
from pathlib import Path

root = Path(sys.argv[1])
update_pipeline = "--update-pipeline" in sys.argv[2:]
contract_path = root / "contract.toml"
with contract_path.open("rb") as f:
    contract = tomllib.load(f)

errors: list[str] = []

commands = set(contract["commands"])
routes = list(contract["routes"])
route_paths = []
for item in routes:
    parts = item.split(None, 1)
    if len(parts) != 2:
        errors.append(f"malformed route: {item!r}")
        continue
    route_paths.append(parts[1])
providers = set(contract["providers"])
tests = contract["tests"]
allow = list(contract["files"])


def allowed_file(rel: str) -> bool:
    for entry in allow:
        if entry.endswith("/"):
            if rel.startswith(entry):
                return True
        elif rel == entry:
            return True
    return False


src_rs: list[Path] = []
src_root = root / "src"
if src_root.is_dir():
    src_rs.extend(sorted(src_root.rglob("*.rs")))
for parent in ("crates", "tools"):
    base = root / parent
    if base.is_dir():
        for crate_src in sorted(base.glob("*/src")):
            src_rs.extend(sorted(crate_src.rglob("*.rs")))

# 1. clap command/subcommand not in [commands]
cmd_new = re.compile(r'Command::new\(\s*"([^"]+)"\s*\)')
found_cmds: set[str] = set()
for path in src_rs:
    text = path.read_text(encoding="utf-8")
    # std::process::Command::new spells the same thing as clap::Command::new;
    # only the latter declares a secd subcommand.
    if "clap" not in text:
        continue
    for name in cmd_new.findall(text):
        if name == "secd":
            continue
        found_cmds.add(name)
extra_cmds = sorted(found_cmds - commands)
if extra_cmds:
    errors.append("clap commands not in contract.toml [commands]: " + ", ".join(extra_cmds))

# 2. axum path not in [routes]
route_lit = re.compile(r'\.route\(\s*"([^"]+)"')
found_routes: set[str] = set()
for path in src_rs:
    text = path.read_text(encoding="utf-8")
    for raw in route_lit.findall(text):
        found_routes.add(raw)
        ok = raw in route_paths or any(p == raw or p.endswith(raw) for p in route_paths)
        if not ok:
            errors.append(f"axum path {raw!r} in {path.relative_to(root)} not in contract.toml [routes]")

# 3. provider name not in [providers]
name_lit = re.compile(
    r'(?:name\s*[:=]\s*|Provider::new\(\s*)"([a-z][a-z0-9]*)"'
)
for path in src_rs:
    if path.name != "provider.rs":
        continue
    text = path.read_text(encoding="utf-8")
    for name in name_lit.findall(text):
        if name not in providers:
            errors.append(f"provider {name!r} in {path.relative_to(root)} not in contract.toml [providers]")

# 4/5. T_ tests vs [tests]
test_fn = re.compile(
    r"#\[test\](?:\s*\n\s*| +)(?:async\s+)?fn\s+(T_[A-Z0-9_]+)\s*\(",
    re.MULTILINE,
)
found_tests: set[str] = set()
for path in root.rglob("*.rs"):
    rel = path.relative_to(root).as_posix()
    if rel.startswith("target/"):
        continue
    text = path.read_text(encoding="utf-8")
    for name in test_fn.findall(text):
        found_tests.add(name)
        if name not in tests:
            errors.append(f"{name} in {rel} is not listed in contract.toml [tests]")

for tid, spec in tests.items():
    pending = bool(spec.get("pending", False)) if isinstance(spec, dict) else False
    if pending:
        continue
    if tid not in found_tests:
        errors.append(f"{tid} listed in contract.toml [tests] has no #[test] fn {tid}")

# 6. file allow-list for src/ and crates/*/src/
for path in src_rs:
    rel = path.relative_to(root).as_posix()
    if not allowed_file(rel):
        errors.append(f"{rel} is not on contract.toml [files] allow-list")

# 7. the pipeline region is closed and content-pinned.
#
# src/ has been closed by [files] since the beginning; .github/, scripts/,
# deploy/, .githooks/ and .claude/ were not closed by anything, and every
# defect the CI/CD audit found lived in exactly that gap. A sha256 per file
# needs no YAML parsing -- which would be the wrong tool for structural
# properties, where a false pass is invisible -- and it fails in about a
# second, offline.
PIPELINE_ROOTS = (
    ".github/",
    "scripts/",
    "deploy/",
    ".githooks/",
    ".claude/",
    # The secret scan's rule set. An allow-list entry added here is a rule the
    # `secrets` lane stops applying, which is exactly the kind of edit that
    # should not pass unremarked.
    ".gitleaks.toml",
    "ui/bunfig.toml",
    "ui/package.json",
)


def tracked_pipeline_files() -> list[str]:
    out = subprocess.run(
        ["git", "-C", str(root), "ls-files", "-z", "--", *PIPELINE_ROOTS],
        capture_output=True,
        check=True,
    ).stdout.decode()
    return sorted(p for p in out.split("\0") if p)


def digest(rel: str) -> str:
    return hashlib.sha256((root / rel).read_bytes()).hexdigest()


def write_pipeline_table(entries: dict[str, str]) -> None:
    body = ["[pipeline]"]
    body += [f'"{name}" = "{sha}"' for name, sha in sorted(entries.items())]
    block = "\n".join(body) + "\n"
    text = contract_path.read_text(encoding="utf-8")
    start = text.find("\n[pipeline]\n")
    if start >= 0:
        start += 1
        end = text.find("\n[", start + 1)
        end = len(text) if end < 0 else end + 1
        text = text[:start] + block + text[end:]
    else:
        anchor = text.find("\n[tests.")
        anchor = len(text) if anchor < 0 else anchor + 1
        text = text[:anchor] + block + "\n" + text[anchor:]
    contract_path.write_text(text, encoding="utf-8")


found_pipeline = tracked_pipeline_files()
if update_pipeline:
    write_pipeline_table({rel: digest(rel) for rel in found_pipeline})
    print(f"plan-contract: pinned {len(found_pipeline)} pipeline files")
    sys.exit(0)

pinned = contract.get("pipeline")
if not isinstance(pinned, dict):
    errors.append("contract.toml has no [pipeline] table; run scripts/check.sh pipeline --update")
else:
    for rel in sorted(set(found_pipeline) - set(pinned)):
        errors.append(f"{rel} is not pinned in contract.toml [pipeline]")
    for rel in sorted(set(pinned) - set(found_pipeline)):
        errors.append(f"{rel} is pinned in contract.toml [pipeline] but is not tracked")
    for rel in sorted(set(found_pipeline) & set(pinned)):
        got = digest(rel)
        if got != pinned[rel]:
            errors.append(
                f"{rel} changed without updating contract.toml [pipeline] "
                f"({pinned[rel][:12]} -> {got[:12]})"
            )

# 8. every `uses:` is pinned to a full commit sha and carries a comment naming
# the version, and no workflow takes an untrusted trigger or interpolates event
# data into a shell command.
USES = re.compile(r"^\s*(?:-\s+)?uses:\s*(\S+)\s*(#.*)?$")
SHA_PIN = re.compile(r"^[\w.-]+/[\w.-]+(?:/[\w.-]+)*@[0-9a-f]{40}$")
BAD_TRIGGERS = ("pull_request_target:", "workflow_run:", "issue_comment:")
INTERP = re.compile(r"\$\{\{\s*(github\.event\b|github\.head_ref\b|inputs\.)")

for rel in found_pipeline:
    if not (rel.endswith(".yml") or rel.endswith(".yaml")):
        continue
    if not rel.startswith(".github/"):
        continue
    text = (root / rel).read_text(encoding="utf-8")
    for trigger in BAD_TRIGGERS:
        if re.search(r"^\s*" + re.escape(trigger), text, re.MULTILINE):
            errors.append(f"{rel} uses {trigger.rstrip(':')}, which runs untrusted refs with write scope")
    in_run = False
    run_indent = 0
    for line in text.splitlines():
        m = USES.match(line)
        if m:
            ref, comment = m.group(1), m.group(2)
            if ref.startswith("./"):
                pass
            elif not SHA_PIN.match(ref):
                errors.append(f"{rel}: `uses: {ref}` is not pinned to a 40-character commit sha")
            elif not comment:
                errors.append(f"{rel}: `uses: {ref}` has no comment naming the version")
        stripped = line.strip()
        indent = len(line) - len(line.lstrip())
        if in_run and stripped and indent <= run_indent:
            in_run = False
        if re.match(r"^\s*(?:-\s+)?run:\s*\|", line):
            in_run = True
            run_indent = indent
            continue
        if in_run and INTERP.search(line):
            errors.append(
                f"{rel}: run: block interpolates event data; pass it through env: instead"
            )

# 9. one version, agreed by every file that restates it.
#
# The release tag is derived from Cargo.toml, and the deploy manifests name an
# image tag built from the same number. Bumping one and not the others used to
# be silent. String equality only: at bump time the new image does not exist
# yet, and a registry lookup here would make the contract need the network.
VERSION_SITES = (
    "crates/secd-web/Cargo.toml",
    "crates/secd-core/Cargo.toml",
    "tools/import-legacy/Cargo.toml",
)


def package_version(rel: str) -> str | None:
    text = (root / rel).read_text(encoding="utf-8")
    m = re.search(r"^\[package\](.*?)(?=^\[|\Z)", text, re.MULTILINE | re.DOTALL)
    if not m:
        return None
    m = re.search(r'^version\s*=\s*"([^"]+)"', m.group(1), re.MULTILINE)
    return m.group(1) if m else None


version = package_version("Cargo.toml")
if not version:
    errors.append("could not read the version from Cargo.toml [package]")
else:
    for rel in VERSION_SITES:
        got = package_version(rel)
        if got != version:
            errors.append(f"{rel} is version {got}, but Cargo.toml is {version}")

    doc = (root / "CLAUDE.md").read_text(encoding="utf-8")
    if f"Version {version}." not in doc:
        errors.append(f"CLAUDE.md does not say `Version {version}.`")

    # Either a tag matching the version or a digest pinned by the release.
    dep = (root / "deploy/k3s/deployment.yaml").read_text(encoding="utf-8")
    if not re.search(
        r"image:\s*\S+secd-web(?::" + re.escape(version) + r"|@sha256:[0-9a-f]{64})\b", dep
    ):
        errors.append(
            f"deploy/k3s/deployment.yaml pins neither :{version} nor a sha256 digest"
        )
    kus = (root / "deploy/k3s/kustomization.yaml").read_text(encoding="utf-8")
    if not re.search(
        r'(?:newTag:\s*"' + re.escape(version) + r'"|digest:\s*sha256:[0-9a-f]{64})', kus
    ):
        errors.append(
            f"deploy/k3s/kustomization.yaml pins neither newTag {version} nor a sha256 digest"
        )

# 10. no job that compiles third-party code holds a secret or a write scope.
#
# release.yml used to set contents: write and packages: write at workflow level
# and pass COSIGN_KEY into the job that runs `cargo build` -- so build.rs from
# every transitive dependency executed with the signing key and a token that
# could force-push to main.
# `cargo install` belongs here: .github/workflows/audit.yml used to run
# `cargo install cargo-audit` in a job holding issues: write, which builds that
# crate's whole dependency tree -- build.rs included -- beside a token that can
# write to this repository. The rule existed and did not see it, because it
# looked for three named scopes and three named cargo subcommands.
COMPILES = re.compile(
    r"cargo\s+(?:build|test|clippy|install|run)\b"
    r"|check\.sh\s+(?:ui|bun-audit|crypto-parity|clippy|test|test-release|compile-fail)\b"
    r"|release\.sh[^\n]*--build-only\b"
    r"|bun\s+(?:install|ci|add|build|run|test)\b"
    r"|bunx\b"
)
# Any write scope and any secret, not an enumeration of the ones seen so far.
# secrets.GITHUB_TOKEN is the exception: its power is exactly the job's
# `permissions:` block, which the first two alternatives already read. Every
# other secret is power the block does not describe -- a GitHub App key mints a
# token whose scopes come from the installation, so a job can write to this
# repository with no `permissions:` line naming write at all.
POWERFUL = re.compile(
    r"^\s*[\w-]+:\s*write\s*$"
    r"|permissions:\s*write-all"
    r"|secrets\.(?!GITHUB_TOKEN\b)[A-Z0-9_]+"
    r"|create-github-app-token",
    re.MULTILINE,
)


def workflow_jobs(rel: str) -> tuple[str, list[tuple[str, str]]]:
    """Everything above `jobs:`, then (name, block) for each job under it.

    Jobs are found by their two-space indent, which is reliable because
    [pipeline] makes every change to these files deliberate. Two-space keys
    appear under `on:` and `concurrency:` as well, so anchor on the `jobs:`
    mapping rather than on indentation alone: without this a workflow_dispatch
    input counts as a job and swallows every real job after it into one block.
    """
    lines = (root / rel).read_text(encoding="utf-8").splitlines()
    try:
        first_job = next(i for i, line in enumerate(lines) if line == "jobs:") + 1
    except StopIteration:
        return "", []
    starts = [
        (i, re.match(r"^  ([\w.-]+):\s*$", line).group(1))
        for i, line in enumerate(lines[first_job:], start=first_job)
        if re.match(r"^  [\w.-]+:\s*$", line)
    ]
    jobs = []
    for n, (start, name) in enumerate(starts):
        end = starts[n + 1][0] if n + 1 < len(starts) else len(lines)
        jobs.append((name, "\n".join(lines[start:end])))
    return "\n".join(lines[: first_job - 1]), jobs


for rel in found_pipeline:
    if not rel.startswith(".github/workflows/"):
        continue
    # A workflow-level permissions block applies to every job in the file, so
    # a compiling job can hold a write scope it never names itself.
    head, jobs = workflow_jobs(rel)
    top_is_powerful = bool(POWERFUL.search(head))
    for job, block in jobs:
        if COMPILES.search(block) and (POWERFUL.search(block) or top_is_powerful):
            errors.append(
                f"{rel}: job `{job}` both compiles and holds a write scope or a "
                f"signing secret; build.rs from every dependency runs in it"
            )

# 11. the gate CI runs is the gate you can run.
#
# The value of splitting check.sh into lanes is that nothing exists in CI that
# cannot be run first on a laptop or in an agent sandbox. That only holds if
# the two lists stay in step: a lane CI never runs is unverified in practice,
# and a lane CI names that check.sh does not know fails only on a runner.
check_sh = (root / "scripts/check.sh").read_text(encoding="utf-8")
m = re.search(r"^ALL_LANES=\(([^)]*)\)", check_sh, re.MULTILINE)
if not m:
    errors.append("scripts/check.sh has no ALL_LANES")
else:
    lanes = set(m.group(1).split())
    fast = re.search(r"fast\) lanes\+=\(([^)]*)\)", check_sh)
    fast_lanes = set(fast.group(1).split()) if fast else set()
    named: set[str] = set()
    for rel in found_pipeline:
        if not rel.startswith(".github/workflows/"):
            continue
        text = (root / rel).read_text(encoding="utf-8")
        for call in re.finditer(r"scripts/check\.sh(?P<args>[^\n|&;]*)", text):
            args = [a for a in call.group("args").split() if not a.startswith("-")]
            if not args or args[0] == "all":
                # A bare run covers every lane, but only where it runs. It is
                # not evidence that the pull-request gate runs them.
                continue
            if args[0] == "pipeline":
                continue
            if args[0] == "fast":
                named.update(fast_lanes)
                continue
            named.update(args)
    unknown = sorted(named - lanes)
    if unknown:
        errors.append(
            "workflows call scripts/check.sh with lanes it does not define: "
            + ", ".join(unknown)
        )
    missing = sorted(lanes - named)
    if missing:
        errors.append(
            "no workflow job names these check.sh lanes: " + ", ".join(missing)
        )

# 12. every checkout drops the token it would otherwise leave behind.
#
# The default leaves GITHUB_TOKEN in .git/config for every later step of the
# job, including cargo, which executes build.rs from every dependency.
for rel in found_pipeline:
    if not (rel.endswith(".yml") or rel.endswith(".yaml")):
        continue
    if not rel.startswith(".github/"):
        continue
    lines = (root / rel).read_text(encoding="utf-8").splitlines()
    for i, line in enumerate(lines):
        if "uses: actions/checkout@" not in line:
            continue
        indent = len(line) - len(line.lstrip())
        window = []
        for follow in lines[i + 1:]:
            if follow.strip() and (len(follow) - len(follow.lstrip())) <= indent:
                break
            window.append(follow)
        if not any("persist-credentials: false" in w for w in window):
            errors.append(
                f"{rel}:{i + 1}: actions/checkout without persist-credentials: false"
            )

# 13. every job that reports on a pull request reaches the gate.
#
# Rule 11 proves a lane is named by some job. It says nothing about whether
# that job's result is one the gate reads, and `gate` is the only context the
# ruleset requires -- so a job left out of its `needs` can fail red while the
# gate reports success and the pull request merges. That is how a required
# check silently becomes advisory, without anyone editing the ruleset.
#
# Jobs that never report on a pull request are exempt, and each one must carry
# an `if:` marker that is false on `pull_request`. Without the marker a new
# job can sit in GATE_EXEMPT, run on a PR, fail, and still leave the gate green.
GATE_EXEMPT = {
    "gate": None,
    "warm": "refs/heads/main",
    "tag": "refs/heads/main",
    "preflight": "refs/tags/v",
    "build": "refs/tags/v",
    "sign": "refs/tags/v",
    "image": "refs/tags/v",
    "publish": "refs/tags/v",
}


def job_needs(block: str) -> set[str]:
    inline = re.search(r"^    needs:\s*\[([^\]]*)\]\s*$", block, re.MULTILINE)
    if inline:
        return {n.strip() for n in inline.group(1).split(",") if n.strip()}
    single = re.search(r"^    needs:\s*([\w.-]+)\s*$", block, re.MULTILINE)
    if single:
        return {single.group(1)}
    listed = re.search(r"^    needs:\s*$", block, re.MULTILINE)
    if not listed:
        return set()
    out: set[str] = set()
    for line in block[listed.end():].splitlines():
        item = re.match(r"^      -\s+([\w.-]+)\s*$", line)
        if item:
            out.add(item.group(1))
        elif line.strip():
            break
    return out


for rel in found_pipeline:
    if not rel.startswith(".github/workflows/"):
        continue
    _, jobs = workflow_jobs(rel)
    names = {name for name, _ in jobs}
    if "gate" not in names:
        errors.append(f"{rel}: no job `gate`; the ruleset requires that check")
        continue
    blocks = dict(jobs)
    for name, marker in GATE_EXEMPT.items():
        if name == "gate":
            continue
        if name not in blocks:
            errors.append(f"{rel}: GATE_EXEMPT job `{name}` is not a job")
            continue
        if marker not in blocks[name]:
            errors.append(
                f"{rel}: exempt job `{name}` is missing if: marker {marker!r}; "
                "without it the job can run on a pull request and still sit "
                "outside gate.needs"
            )
    gate_block = blocks["gate"]
    needs = job_needs(gate_block)
    unreached = sorted(names - set(GATE_EXEMPT) - needs)
    if unreached:
        errors.append(
            f"{rel}: job `gate` does not depend on "
            + ", ".join(unreached)
            + "; a job outside gate.needs can fail while the gate reports success"
        )
    phantom = sorted(needs - names)
    if phantom:
        errors.append(
            f"{rel}: job `gate` depends on " + ", ".join(phantom) + ", which is not a job"
        )

# 14. one workflow file.
#
# GitHub treats every YAML under .github/workflows/ as a named workflow (a
# notification stream, a check, a bill). Jobs skip by if:; they are not split
# into files. A generator that writes a second system is a liability.
wf = [p for p in found_pipeline if p.startswith(".github/workflows/")]
if wf != [".github/workflows/ci.yml"]:
    errors.append(
        "exactly one workflow file allowed (.github/workflows/ci.yml); found: "
        + (", ".join(wf) if wf else "none")
    )

if errors:
    sys.stderr.write("plan-contract:\n")
    for e in errors:
        sys.stderr.write(f"  {e}\n")
    sys.exit(1)
PY
