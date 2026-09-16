# Analysis reference

Detailed behavior and supported scope for Oxarch. For installation and common workflows, see the [README](../README.md).

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

## Current limitations

- Entry points and dead-module candidates use filename heuristics; no framework-specific reachability model is implied.
- Module size is measured in lines, not cyclomatic complexity.
- Dynamic runtime expressions cannot be resolved. Bare `require` calls are detected syntactically, without checking whether that identifier is locally shadowed.
- Vue extraction covers inline script blocks, not a full single-file-component compiler or external `src` scripts.
- TypeScript/workspace resolution handles common layouts, not every Node/TypeScript resolution mode. Package `exports` conditions, project references, package-based tsconfig extensions, and `.mts`/`.cts` sources are not implemented. Internal package discovery is limited to four directory levels and assumes `src/index` or `src/<subpath>` entry points.
- TypeScript parse recovery is best effort; this tool does not replace a compiler/type checker.
- The explorer is a local, single-request-at-a-time development server, not a public hosting service.
- The graph is paginated for usability; very large graphs may require focused filtering.
