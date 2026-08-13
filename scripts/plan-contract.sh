#!/usr/bin/env bash
set -euo pipefail
root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$root"
exec python3 - "$root" <<'PY'
import re
import sys
import tomllib
from pathlib import Path

root = Path(sys.argv[1])
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
crates = root / "crates"
if crates.is_dir():
    for crate_src in sorted(crates.glob("*/src")):
        src_rs.extend(sorted(crate_src.rglob("*.rs")))

# 1. clap command/subcommand not in [commands]
cmd_new = re.compile(r'Command::new\(\s*"([^"]+)"\s*\)')
found_cmds: set[str] = set()
for path in src_rs:
    text = path.read_text(encoding="utf-8")
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

if errors:
    sys.stderr.write("plan-contract:\n")
    for e in errors:
        sys.stderr.write(f"  {e}\n")
    sys.exit(1)
PY
