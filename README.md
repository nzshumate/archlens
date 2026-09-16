# Archlens

> **See your frontend architecture.**

Archlens is a Rust-powered architecture analyzer for TypeScript, React, and Vue frontends. It maps module dependencies, detects structural problems, enforces architecture boundaries, analyzes Git changes, and includes a local architecture explorer.

## What it does

- Scans `.ts`, `.tsx`, `.js`, `.jsx`, `.vue`, `.mjs`, and `.cjs`
- Parses ES module imports/re-exports plus dynamic `import()` and CommonJS `require()`
- Extracts Vue `<script>` and `<script setup>` content
- Resolves relative imports, index modules, TypeScript aliases, extended/JSONC tsconfig files, and internal workspace packages
- Builds a directed dependency graph and detects cycles
- Calculates fan-in/fan-out, dead-module candidates, large-module/complexity signals, and an architecture health score
- Enforces configurable layer/boundary rules from `archlens.json`
- Reports Git branch impact with `archlens diff <base>`
- Provides CI-friendly `archlens check` exit codes
- Runs source parsing in parallel with Rayon
- Includes an interactive local explorer via `archlens dev`
- Includes unit/integration coverage and GitHub Actions validation

## Why Rust?

Architecture analysis is filesystem-, parsing-, graph-, and CPU-heavy work. Rust gives Archlens a native core with predictable memory use, safe parallelism, and a standalone binary that is well suited to local development and CI. Archlens deliberately avoids publishing performance claims until they are backed by reproducible benchmark results.

## Install

```bash
git clone https://github.com/nzshumate/archlens.git
cd archlens
cargo build --release
```

The binary is available at `target/release/archlens`.

## CLI

Analyze a repository:

```bash
archlens analyze ./path/to/frontend
```

Emit the complete machine-readable report:

```bash
archlens analyze ./path/to/frontend --json
```

Enforce architecture health in CI:

```bash
archlens check . --min-health 80
```

Allow existing cycles while still enforcing health and boundary rules:

```bash
archlens check . --min-health 80 --allow-cycles
```

Inspect architectural impact relative to a Git ref:

```bash
archlens diff main .
archlens diff main . --json
```

Launch the local explorer:

```bash
archlens dev . --port 4242
```

Then open `http://127.0.0.1:4242`.

## Architecture rules

Create `archlens.json` at the repository root:

```json
{
  "boundaries": [
    {
      "from": "src/ui/",
      "disallow": "src/data/",
      "message": "UI modules must use the service layer instead of importing data modules directly"
    }
  ]
}
```

`archlens check` exits non-zero when a configured boundary is violated, the health score falls below the requested threshold, or cycles are present unless `--allow-cycles` is supplied.

See `archlens.example.json` for a copyable example.

## Architecture

```text
Frontend repository
        |
        v
 filesystem discovery
        |
        v
 parallel source parsing
      (Oxc)
        |
        v
 import resolution
 relative / tsconfig / workspace
        |
        v
 directed dependency graph
        |
        +--> cycle detection
        +--> fan-in / fan-out
        +--> dead-code candidates
        +--> complexity signals
        +--> boundary rules
        +--> Git impact analysis
        |
        +--> CLI / JSON / CI
        |
        `--> local explorer
```

## Commands

| Command | Purpose |
| --- | --- |
| `archlens analyze [path]` | Analyze a frontend repository |
| `archlens analyze [path] --json` | Emit the complete graph and metrics |
| `archlens check [path]` | Enforce architecture requirements in CI |
| `archlens diff <base> [path]` | Show architectural impact since a Git base ref |
| `archlens dev [path]` | Launch the local architecture explorer |

## Roadmap status

The original MVP roadmap is implemented:

### Analysis core

- [x] Native Rust CLI
- [x] Source-file discovery
- [x] Oxc-based source parsing
- [x] Vue script extraction
- [x] Static imports and re-exports
- [x] Dynamic `import()` and CommonJS `require()` discovery
- [x] Dependency graph construction
- [x] Circular dependency detection
- [x] TypeScript path aliases
- [x] JSONC and extended tsconfig support
- [x] Internal workspace/package resolution
- [x] JSON graph output
- [x] Parser, metric, rule, and integration tests

### Architecture intelligence

- [x] Dead/unreachable-module candidates
- [x] High fan-in/fan-out modules
- [x] Configurable architecture boundaries
- [x] Layer violations
- [x] Module size/complexity signals
- [x] Architecture health summary

### Git-aware analysis

- [x] `archlens diff <base>`
- [x] Changed-source detection
- [x] Affected dependency reporting
- [x] Current-tree cycle impact
- [x] CI-friendly exit codes and output

### Explorer

- [x] `archlens dev`
- [x] Local architecture dashboard
- [x] Module search/filtering
- [x] Dependency/fan-in/fan-out exploration
- [x] Cycle and health visibility

### Performance and quality

- [x] Parallel source analysis with Rayon
- [x] Benchmark harness for repeatable measurements
- [x] CI formatting, Clippy, tests, and end-to-end self-analysis
- [x] No unverified performance claims in documentation

Incremental on-disk caching and richer graph-layout/branch-diff visualization are intentionally tracked as post-MVP optimization work rather than prerequisites for the original usable release.

## Verification

GitHub Actions validates the project on every push and pull request by normalizing formatting, running Clippy with warnings denied, running the complete test suite, and running Archlens against its own repository. The project is not considered releasable when that pipeline is red.

## Design principles

1. **Measure, don't market.** Publish benchmark numbers only when they are reproducible.
2. **Useful without a UI.** CLI and JSON are first-class interfaces.
3. **Understand modern frontends.** TypeScript, Vue, React, aliases, and workspaces are core use cases.
4. **Actionable analysis.** Surface structural problems developers can act on, not merely a decorative graph.
5. **Fast enough for every PR.** CI is a primary use case.
6. **Deterministic before AI.** Core architecture analysis should remain explainable and reproducible.

## Current limitations

Archlens is still pre-1.0. Dead-module detection is heuristic because application entry points vary by framework. The health score is an opinionated signal rather than a universal measure of code quality. Workspace and tsconfig resolution cover common frontend layouts but do not yet attempt to reproduce every edge case in Node/TypeScript module resolution.

## License

MIT
