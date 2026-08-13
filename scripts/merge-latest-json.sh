#!/usr/bin/env bash
# Union latest.json targets. Every input must share the same version.
set -euo pipefail

out=""
files=()
while [[ $# -gt 0 ]]; do
  case "$1" in
    -o)
      out="${2:?merge-latest-json: -o needs a path}"
      shift 2
      ;;
    -*)
      echo "merge-latest-json: usage: merge-latest-json.sh [-o out] a.json b.json ..." >&2
      exit 2
      ;;
    *)
      files+=("$1")
      shift
      ;;
  esac
done

if [[ ${#files[@]} -lt 1 ]]; then
  echo "merge-latest-json: usage: merge-latest-json.sh [-o out] a.json b.json ..." >&2
  exit 2
fi

python3 - "$out" "${files[@]}" <<'PY'
import json
import sys

out = sys.argv[1]
paths = sys.argv[2:]
version = None
targets: dict = {}

for path in paths:
    with open(path, encoding="utf-8") as f:
        doc = json.load(f)
    if not isinstance(doc, dict):
        raise SystemExit(f"merge-latest-json: {path} is not an object")
    ver = doc.get("version")
    if ver is None:
        raise SystemExit(f"merge-latest-json: {path} has no version")
    if version is None:
        version = ver
    elif ver != version:
        raise SystemExit(
            f"merge-latest-json: version {ver!r} in {path} != {version!r}"
        )
    chunk = doc.get("targets")
    if not isinstance(chunk, dict):
        raise SystemExit(f"merge-latest-json: {path} has no targets object")
    for triple, spec in chunk.items():
        if not isinstance(spec, dict):
            raise SystemExit(f"merge-latest-json: {path} target {triple} is not an object")
        for key in ("url", "sha256", "sig"):
            if key not in spec:
                raise SystemExit(
                    f"merge-latest-json: {path} target {triple} missing {key}"
                )
        targets[triple] = {
            "sha256": spec["sha256"],
            "sig": spec["sig"],
            "url": spec["url"],
        }

if not version or not targets:
    raise SystemExit("merge-latest-json: nothing to merge")

doc = {"targets": targets, "version": version}
text = json.dumps(doc, indent=2, sort_keys=True) + "\n"
if out:
    with open(out, "w", encoding="utf-8") as f:
        f.write(text)
else:
    sys.stdout.write(text)
PY
