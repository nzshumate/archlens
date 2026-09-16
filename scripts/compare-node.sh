#!/usr/bin/env bash
set -euo pipefail
TARGET="${1:-.}"
cargo build --release --locked
# POSIX time works on macOS and Linux. These tools have different semantics;
# this is an exploratory comparison, not a like-for-like speed claim.
printf 'Oxarch (release, uncached)\n'
time -p ./target/release/oxarch analyze "$TARGET" --no-cache --json >/dev/null
if command -v npx >/dev/null 2>&1; then
  printf 'madge (includes npx startup; may include package download)\n'
  time -p npx --yes madge "$TARGET" --extensions ts,tsx,js,jsx --json >/dev/null
else
  printf 'npx unavailable; skipping Node comparison\n'
fi
