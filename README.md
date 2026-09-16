# Archlens

> **See your frontend architecture.**

Archlens is a Rust-powered architecture analyzer for TypeScript frontends that maps dependencies, detects structural problems, and shows how code changes affect your application.

Archlens is early-stage and under active development. The goal is a fast native developer tool that can analyze large TypeScript, React, and Vue codebases locally and in CI, then expose the resulting architecture graph to an interactive frontend.

## Why Rust?

Architecture analysis is filesystem-, parsing-, and graph-heavy work. Rust gives Archlens a native core with predictable memory use, strong concurrency options, and the ability to ship as a standalone binary. Performance claims will be benchmarked rather than assumed.

## Current capabilities

- Scans `.ts`, `.tsx`, `.js`, `.jsx`, `.vue`, `.mjs`, and `.cjs`
- Ignores generated/vendor directories such as `node_modules`, `dist`, `.next`, `.nuxt`, and `coverage`
- Parses ES module imports and re-exports with the Oxc parser rather than regex
- Extracts imports from Vue script blocks
- Resolves relative imports and common index-file imports
- Reads `baseUrl` and `paths` from `tsconfig.json`
- Builds a directed module dependency graph
- Detects circular dependency groups
- Emits graph nodes and edges as JSON for visualization and integrations
- Runs formatting, Clippy, and tests in GitHub Actions

## Install and run

You need a current Rust toolchain.

```bash
cargo build --release
./target/release/archlens analyze ./path/to/frontend
```

During development:

```bash
cargo run -- analyze ./path/to/frontend
```

Get the full dependency graph as JSON:

```bash
cargo run -- analyze ./path/to/frontend --json
```

## Architecture

```text
Frontend repository
        |
        v
  filesystem scanner
        |
        v
   Oxc AST parser
        |
        v
 import resolver
(relative + tsconfig aliases)
        |
        v
 dependency graph
        |
        +--> cycle analysis
        +--> architecture rules
        +--> diff analysis
        +--> JSON graph API
                  |
                  v
          interactive UI
```

The Rust core owns scanning, parsing, resolution, graph construction, and analysis. The planned web UI will focus on exploration and visualization rather than duplicating analysis logic in JavaScript.

## Planned CLI

```text
archlens analyze [path]       Analyze a repository
archlens analyze --json       Emit the complete graph
archlens diff <base>          Show architectural impact of a branch
archlens check                Enforce architecture rules in CI
archlens dev                  Launch the interactive architecture explorer
```

`diff`, `check`, and `dev` are roadmap commands and are not implemented yet.

## Roadmap

### 0.1 — analysis core

- [x] Native Rust CLI
- [x] Source-file discovery
- [x] AST-based ES module parsing
- [x] Vue script extraction
- [x] Dependency graph construction
- [x] Circular dependency detection
- [x] Basic `tsconfig` aliases
- [x] JSON graph output
- [x] GitHub Actions validation
- [ ] Dynamic `import()` and CommonJS AST support
- [ ] Full JSONC/extended tsconfig resolution
- [ ] Workspace/package resolution
- [ ] Parser and resolver fixture suite

### 0.2 — architecture intelligence

- [ ] Dead/unreachable modules
- [ ] High fan-in/fan-out modules
- [ ] Configurable architecture boundaries
- [ ] Layer violations
- [ ] Component/module complexity metrics
- [ ] Dependency health summary

### 0.3 — Git-aware analysis

- [ ] `archlens diff <base>`
- [ ] New/removed dependencies by branch
- [ ] Newly introduced cycles
- [ ] Architecture impact report
- [ ] CI-friendly exit codes and output

### 0.4 — explorer

- [ ] `archlens dev`
- [ ] Interactive dependency graph
- [ ] Search/filter modules
- [ ] Cycle visualization
- [ ] Architecture health dashboard
- [ ] Branch-impact visualization

### 0.5 — performance

- [ ] Parallel file analysis
- [ ] Incremental analysis/cache
- [ ] Benchmark corpus
- [ ] Comparisons against equivalent JS/Node tooling

## Design principles

1. **Measure, don't market.** Performance numbers belong in this README only after reproducible benchmarks exist.
2. **Useful without a UI.** The CLI and JSON output should remain first-class interfaces.
3. **Understand modern frontends.** Vue, React, TypeScript aliases, monorepos, and real-world module resolution are core use cases.
4. **Actionable analysis.** Archlens should explain architectural problems and their source, not just draw a pretty graph.
5. **Fast enough for every PR.** CI analysis is a primary use case, not an afterthought.

## Status

Experimental. APIs, output formats, and commands may change before the first stable release.

## License

MIT
