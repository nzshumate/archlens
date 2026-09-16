#!/usr/bin/env bash
set -euo pipefail
TARGET="${1:-.}"
RUNS="${RUNS:-5}"
cargo build --release >/dev/null
printf 'Archlens benchmark: %s (%s runs)\n' "$TARGET" "$RUNS"
for i in $(seq 1 "$RUNS"); do
  /usr/bin/time -f "run $i: %e sec, %M KB max RSS" ./target/release/archlens analyze "$TARGET" --json >/dev/null
done
