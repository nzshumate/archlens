# Oxarch

> **See your frontend architecture.**

Oxarch is a Rust-powered architecture analyzer for TypeScript, React, and Vue frontends. It maps module dependencies, detects structural problems, enforces architecture boundaries, compares Git branches, and includes a local interactive explorer.

The original MVP and the previously deferred caching and graph/branch-visualization roadmap are implemented. Oxarch remains pre-1.0; the limitations below define the supported scope.

## Quick start

Requirements: a current stable Rust toolchain; Git for branch comparisons; Python 3 for the optional benchmark scripts. No Node runtime is required to run Oxarch or its explorer.

```bash
git clone https://github.com/nzshumate/oxarch.git
cd oxarch
cargo build --release --locked

./target/release/oxarch analyze /path/to/frontend
./target/release/oxarch dev /path/to/frontend --port 4242
```

Open `http://127.0.0.1:4242`. The explorer binds only to the loopback interface and bundles its assets into the binary; it does not require a CDN or upload source files.

To install the CLI on your PATH:

```bash
cargo install --path . --locked
```

## Commands

| Command | Purpose |
| --- | --- |
| `oxarch analyze [path]` | Print graph counts, health, and structural findings |
| `oxarch analyze [path] --json` | Emit analysis, metrics, and boundary violations as JSON |
| `oxarch analyze [path] --no-cache` | Analyze without reading or writing the parser cache |
| `oxarch check [path] --min-health 80` | Enforce boundaries, health, and absence of cycles |
| `oxarch check [path] --allow-cycles` | Allow cycles while enforcing health and boundaries |
| `oxarch diff <base> [path] --json` | Compare the current tree against the branch merge base |
| `oxarch dev [path] --port 4242` | Launch the local explorer |
| `oxarch dev [path] --base main` | Explore the graph with branch-impact overlays |

The default path is `.`. `check` defaults to a minimum health score of 70; accepted thresholds are 0–100. Failed checks, unreadable source files, invalid architecture rules, and Git errors produce a nonzero exit status.

## Analysis

- Discovers `.ts`, `.tsx`, `.js`, `.jsx`, `.vue`, `.mjs`, and `.cjs` source files.
- Uses Oxc to parse static imports, re-exports, literal dynamic `import()`, and bare `require()` calls. Comments and ordinary strings do not create dependencies.
- Extracts inline Vue `<script>` and `<script setup>` blocks.
- Resolves relative paths, dotted module names such as `./user.service`, index modules, TypeScript path aliases, JSONC/relative tsconfig extensions, and common internal workspace packages.
- Parses sources in parallel with Rayon, then builds a directed graph with one edge per importing/imported module pair.
- Detects strongly connected cycle groups, including self-imports. A group is a set of mutually reachable modules, not a guaranteed ordered cycle path.
- Reports fan-in/fan-out hubs, orphan/dead-module candidates, modules with at least 300 lines, and an architecture health score.

Generated/vendor directories named `node_modules`, `.git`, `dist`, `build`, `.next`, `.nuxt`, `coverage`, and `target` are skipped. Discovery does not otherwise follow `.gitignore` patterns.

The health score subtracts penalties for modules in cycles, dead-module candidates, and large modules. It is an opinionated signal, not a universal quality rating. Boundary violations are enforced separately.

## Architecture boundaries

Create `oxarch.json` at the analysis root:

```json
{
  "boundaries": [
    {
      "from": "src/ui/",
      "disallow": "src/data/",
      "message": "UI modules must use the service layer"
    }
  ]
}
```

Rules match path prefixes. A trailing `/` makes a directory boundary explicit. `oxarch check` fails if any matching dependency violates a rule, even when cycles are allowed or the health score is high. Missing configuration means no boundary rules; malformed configuration is an error, not a silent pass.

See [oxarch.example.json](oxarch.example.json) for a copyable example.

## Incremental cache

Every normal analysis maintains `target/oxarch/parsed-v1.json` inside the analyzed project. Add `target/` to that project's `.gitignore` if needed.

- Cached entries contain source text, extracted imports, and line counts.
- Exact content comparison reuses parsing results for unchanged files; modified and new files are parsed again, and deleted entries are removed.
- It works in dirty Git worktrees and directories without Git.
- Import resolution, configuration/workspace discovery, graph construction, metrics, and rules run fresh every time. Changing a tsconfig alias cannot leave an old resolved edge in the report.
- Cache writes are atomic. Missing, corrupt, incompatible, or unwritable caches do not prevent analysis.
- `--no-cache` bypasses the cache for independent validation and uncached measurements. Removing `target/oxarch` resets it.

This is incremental **parsing**, not a filesystem watcher or an incremental graph engine. Files are still read to validate their contents. The cache stores local source text; treat it like the source tree and do not publish it.

## Git branch impact

```bash
oxarch diff main /path/to/frontend
oxarch diff main /path/to/frontend --json
oxarch dev /path/to/frontend --base main
```

Oxarch resolves `main` to a commit and compares the current working tree with `git merge-base main HEAD`. This excludes changes made only on the base branch after divergence. Staged, unstaged, and newly discovered source files are included; the analysis root can be a subdirectory of the Git repository.

The report includes:

- Merge-base commit and changed source files, including deleted files.
- Added and removed dependencies, both as readable labels and structured `added_edges`/`removed_edges` arrays.
- Deleted modules, new cycle groups, and current cycle count.
- Transitively affected importing modules across both the previous and current graphs. Resolution-only edge changes also seed this impact calculation.

`affected_dependencies` counts distinct previous/current edges touching the affected module set. Renames appear as deletion plus addition. A detached temporary worktree supplies the base snapshot and is removed after analysis, including when analysis fails. Git must permit creating temporary worktrees.

## Interactive explorer

The explorer includes:

- Health, module, dependency, cycle, and boundary-violation summaries.
- Dependency-layer and circular layouts with directed arrows.
- Clickable and keyboard-selectable nodes, incoming/outgoing dependency details, and removed connections.
- Search and all/cycle/changed/affected filters.
- Drag-to-pan, scroll/button zoom, and fit-to-graph controls.
- Added edges in green; removed edges and deleted nodes in dashed pink; cycle borders in amber.
- Branch summaries, changed modules, cycle groups, and boundary explanations.
- **Refresh analysis** to reread sources and recompute the report without restarting the server.

The graph and module table show 100 matching modules per page; previous/next controls expose all results. Edges are drawn only when both endpoints are visible on that page. Module details still list all its current connections. Use search to narrow a large project. Changed/affected views require `--base`.

The explorer analyzes on demand, not automatically on filesystem changes. Filenames, rule messages, and branch labels are rendered as text rather than HTML. API failures are shown in the status area and can be retried with Refresh.

## Architecture

```text
Source discovery → content-validated parser cache → parallel Oxc parsing
                         ↓
        fresh import resolution (relative / tsconfig / workspace)
                         ↓
              directed dependency graph
                         ↓
       cycles / metrics / boundaries / Git impact
                         ↓
                CLI · JSON · CI · explorer
```

## Roadmap status

### Analysis core

- [x] Native Rust CLI and source discovery
- [x] Oxc parsing, Vue script extraction, imports and re-exports
- [x] Literal dynamic imports and CommonJS calls
- [x] Relative/index/dotted imports, TypeScript aliases, JSONC and relative tsconfig extensions
- [x] Common internal workspace package resolution
- [x] Dependency graph, cycle groups, self-import detection, and JSON output
- [x] Parser, resolver, metric, rule, and CLI regression tests

### Architecture intelligence

- [x] Orphan/dead-module candidates and fan-in/fan-out hubs
- [x] Configurable layer/boundary rules with CI enforcement
- [x] Module size signals and architecture health summary

### Git-aware analysis

- [x] Merge-base comparison with `oxarch diff <base>`
- [x] Changed/deleted/uncommitted source files and nested analysis roots
- [x] Added/removed dependencies and new cycle groups
- [x] Transitive importing-module impact
- [x] CI exit codes and machine-readable reports

### Explorer

- [x] Local architecture dashboard, search, and module details
- [x] Dependency and circular graph layouts, node selection, pan/zoom/fit
- [x] Cycle, health, and boundary visibility
- [x] Branch-diff graph overlays and deleted modules
- [x] Filters, pagination, and on-demand refresh

### Performance and quality

- [x] Parallel parsing with Rayon
- [x] Content-validated incremental on-disk parser cache
- [x] Corruption recovery and uncached validation mode
- [x] Synthetic corpus generator and portable cold/warm benchmark harness
- [x] Locked dependencies and strict formatting, Clippy, tests, and self-analysis in CI
- [x] Linux/macOS CI configuration and cold/warm result equivalence check
- [x] No unsupported performance claims

## Verification

```bash
cargo fmt --all --check
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo test --locked --all
cargo run --locked -- analyze . --json
node --check src/explorer.js
python3 scripts/smoke-explorer.py
```

The GitHub Actions workflow runs these checks on Linux and macOS, plus HTTP asset/routing/refresh/error-recovery checks and a generated-corpus cold/warm equivalence check. Self-analysis is a smoke test; the integration fixtures provide meaningful frontend dependency coverage. Local verification does not imply a remote GitHub Actions run has passed.

Tests cover dotted imports, parser false positives, JSONC strings, cache edits/deletions/config changes/corruption, self-cycles, boundary enforcement, refresh behavior, and divergent/nested/dirty Git comparisons with worktree cleanup.

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

## Current limitations

- Entry points and dead-module candidates use filename heuristics; no framework-specific reachability model is implied.
- Module size is measured in lines, not cyclomatic complexity.
- Dynamic runtime expressions cannot be resolved. Bare `require` calls are detected syntactically, without checking whether that identifier is locally shadowed.
- Vue extraction covers inline script blocks, not a full single-file-component compiler or external `src` scripts.
- TypeScript/workspace resolution handles common layouts, not every Node/TypeScript resolution mode. Package `exports` conditions, project references, package-based tsconfig extensions, and `.mts`/`.cts` sources are not implemented. Internal package discovery is limited to four directory levels and assumes `src/index` or `src/<subpath>` entry points.
- TypeScript parse recovery is best effort; this tool does not replace a compiler/type checker.
- The explorer is a local, single-request-at-a-time development server, not a public hosting service.
- The graph is paginated for usability; very large graphs may require focused filtering.

## License

Copyright 2026 Nathan Shumate. Licensed under the [Apache License, Version 2.0](LICENSE).
