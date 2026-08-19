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
PIPELINE_ROOTS = (".github/", "scripts/", "deploy/", ".githooks/", ".claude/")


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
    "crates/secd-ui/Cargo.toml",
    "crates/secd-web/Cargo.toml",
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

if errors:
    sys.stderr.write("plan-contract:\n")
    for e in errors:
        sys.stderr.write(f"  {e}\n")
    sys.exit(1)
PY
