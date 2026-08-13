#!/usr/bin/env bash
set -euo pipefail
: "${CARGO_TARGET_DIR:?ci-cache: CARGO_TARGET_DIR required}"
cap="${SECD_CACHE_CAP_GB:-20}"
if [[ -d "$CARGO_TARGET_DIR" ]]; then
  used_gb=$(du -sBG "$CARGO_TARGET_DIR" | awk '{gsub(/G/,"",$1); print $1}')
  if [[ "${used_gb:-0}" -gt "$cap" ]]; then
    echo "build cache ${used_gb}G exceeds ${cap}G cap — wiping $CARGO_TARGET_DIR"
    rm -rf "$CARGO_TARGET_DIR"
  else
    echo "build cache ${used_gb}G of ${cap}G cap"
  fi
else
  echo "build cache empty ($CARGO_TARGET_DIR)"
fi
