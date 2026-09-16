# Contributing

Clone the repository and install a current stable Rust toolchain with `rustfmt` and `clippy`. Node.js is used only to check explorer JavaScript syntax; Python 3 runs the HTTP smoke test and benchmarks. Git is required by branch-comparison tests.

## Development checks

```bash
cargo fmt --all --check
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo test --locked --all
cargo run --locked -- analyze . --json
node --check src/explorer.js
python3 scripts/smoke-explorer.py
```

The HTTP smoke test starts a temporary loopback server and stops it when finished. GitHub Actions runs the checks on Linux and macOS and compares cached and uncached reports on a generated source corpus.

Add regression coverage when changing parsing, import resolution, caching, rules, or Git comparisons. Explorer changes should also be checked in a browser, including filtering, node selection, refresh, and error states. Changes to cached parsing semantics require a cache format/version update.

## Reproducible benchmarks

```bash
python3 scripts/generate-corpus.py 5000 /tmp/oxarch-corpus
RUNS=5 bash scripts/benchmark.sh /tmp/oxarch-corpus
```

The harness builds the release binary and prints JSON with each run's wall time and the median for uncached and warm-cache analysis. It works on macOS and Linux. Parsing is only part of total analysis cost, so warm caching is not guaranteed to improve every workload.

For an existing binary:

```bash
python3 scripts/benchmark.py /tmp/oxarch-corpus --binary target/release/oxarch --runs 5
```

An optional `bash scripts/compare-node.sh /path/to/frontend` runs Oxarch and Madge. It may download Madge through `npx`; the tools have different analysis semantics, and its timings include process startup. Do not present this as a like-for-like speed claim.

## Pull requests

Describe the user-visible change, include the relevant verification results, and update the documentation when behavior changes. Keep performance claims tied to reproducible measurements and identify the workload and environment.
