#!/usr/bin/env bash
set -euo pipefail
TARGET="${1:-.}"
cargo build --release >/dev/null
echo "Archlens (release)"
/usr/bin/time -f '%e sec, %M KB max RSS' ./target/release/archlens analyze "$TARGET" --json >/dev/null
if command -v npx >/dev/null 2>&1; then
  echo "madge (npx, dependency graph JSON)"
  /usr/bin/time -f '%e sec, %M KB max RSS' npx --yes madge "$TARGET" --json >/dev/null
else
  echo "npx unavailable; skipping Node comparison"
fi
