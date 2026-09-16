# Analysis reference

Detailed behavior and supported scope for Oxarch. For installation and common workflows, see the [README](../README.md).

## Analysis

- Discovers `.ts`, `.tsx`, `.mts`, `.cts`, `.js`, `.jsx`, `.vue`, `.mjs`, and `.cjs` source files.
- Uses Oxc to parse static imports, re-exports, literal dynamic `import()`, and bare `require()` calls. Comments and ordinary strings do not create dependencies.
- Parses inline Vue `<script>` and `<script setup>` blocks separately, including JSX/TSX, and follows literal external `src` scripts. Parse locations refer to the original `.vue` file.
- Resolves relative paths, dotted module names such as `./user.service`, index modules, TypeScript path aliases, JSONC/relative tsconfig extensions, and common internal workspace packages.
- Parses sources in parallel with Rayon, then builds a directed graph with one edge per importing/imported module pair.
- Detects strongly connected cycle groups, including self-imports. A group is a set of mutually reachable modules, not a guaranteed ordered cycle path.
- Reports fan-in/fan-out hubs, orphan/dead-module candidates, modules with at least 300 lines, and an architecture health score.

Discovery honors `.gitignore` and `.oxarchignore` in the chosen analysis root and its descendants, even outside a Git repository. Patterns use Git ignore syntax, including negation; ignored parent directories must be unignored before their children can be included. `.oxarchignore` takes precedence over `.gitignore` at the same level. Ignore rules apply to tracked files too. Parent-directory/global Git ignore settings and `.git/info/exclude` are not read, so results depend on the chosen project rather than developer-specific settings. Symlinks are not followed.

Generated/vendor directories named `node_modules`, `.git`, `dist`, `build`, `.next`, `.nuxt`, `coverage`, and `target` are always skipped. Ignore rules affect source, config, and package discovery; an explicitly referenced relative config extension is still loaded. Invalid ignore patterns, unreadable files, or malformed discovered configuration files fail analysis.

The health score subtracts penalties for modules in cycles, dead-module candidates, and large modules. It is an opinionated signal, not a universal quality rating. Boundary violations are enforced separately.

## Resolution

Each source uses the nearest discovered `tsconfig.json` or `jsconfig.json` up to the analysis root; TypeScript config takes precedence when both exist. JSONC comments and trailing commas are supported. Relative `extends` chains preserve configuration origins and reject missing files, cycles, or chains longer than 32 levels. Package extensions resolve through ancestor `node_modules` directories, including scoped names, explicit subpaths (with optional `.json`), a package’s `tsconfig` field, and default `tsconfig.json`. Multiple `extends` entries are applied left to right, with later settings overriding earlier ones. No packages are downloaded or executed. Missing shared package configs produce `unresolved_config` diagnostics while local settings are still used; `check` fails even without `--strict`. Missing relative files, malformed configs, and extension cycles remain fatal. Project references, package exports maps for config lookup, and Yarn Plug’n’Play are not resolved.

`baseUrl` and `paths` support exact aliases and one wildcard, including suffix patterns. Relative imports support index files and dotted names. Runtime extension substitution maps `.js` to `.ts`/`.tsx`, `.jsx` to `.tsx`, `.mjs` to `.mts`, and `.cjs` to `.cts` before considering the corresponding JavaScript file. Query suffixes are stripped for resolution.

Internal packages are discovered from non-ignored `package.json` files without a depth limit. Duplicate names and malformed manifests fail analysis. Root imports prefer an explicit `source` field. Otherwise `exports` supports root, exact subpath, wildcard, array, and condition targets; unexported private subpaths stay unresolved. Conditions use this static preference: `source`, `browser`, `import`, `default`, `require`, then `types`. This is an analysis convention, not full Node.js runtime resolution. Without `exports`, root candidates are `module`, `main`, `src/index`, then `index`; subpaths try the package directory and then `src`.

Targets must be scanned source files to become graph edges. Generated outputs, ignored files, files outside the root, and external dependencies do not become nodes. For source packages whose exports point at build output, provide source-oriented exports/aliases or a root `source` field.

## Reachability and diagnostics

Set `entryPoints` in `oxarch.json` to explicit root-relative source paths, for example:

```json
{"entryPoints": ["src/main.ts", "src/worker.ts"], "boundaries": []}
```

Every entry must exist in the scanned graph. Reachability follows outgoing imports from all entries and reports unreachable modules in `metrics.dead_candidates`, including isolated cycles. Explicit entries override automatic detection. Otherwise, Next.js and Expo conventions supply roots when available; remaining projects use the zero-incoming-edge and main/index/app filename heuristic. `metrics.reachability_mode` identifies `explicit`, `framework`, or `heuristic` analysis. Declaration files (`.d.ts`, `.d.mts`, `.d.cts`) stay in the graph but are excluded from unused-module candidates. Reachability is module-level, not unused-export detection; type-only imports count as dependencies. Custom framework routing, test discovery, scripts, and runtime loading can make apparently unreachable code useful.

Framework detection requires `next` or `expo` in a discovered package's dependencies, devDependencies, or peerDependencies. Each source belongs to its nearest package, so conventions do not leak into unrelated nested packages. `analysis.framework_entry_points` exposes the detected roots even when explicit roots override them.

- Next.js: standard `app`/`src/app` special files (pages, layouts, handlers, error/loading boundaries, metadata routes), Pages Router files, middleware/proxy/instrumentation files, and `next.config.js`/`.mjs`/`.ts`. App Router private folders beginning with `_` are excluded. Custom `pageExtensions` and executable config overrides are not interpreted.
- Expo: the exact local `main` source path, or conventional index/App files when no main is declared. With `expo-router/entry` and an `expo-router` dependency, files under `app`/`src/app` are route roots.
- Only in a detected Next.js package's root `next-env.d.ts`, missing `./.next/types/*.d.ts` and `./.next/dev/types/*.d.ts` imports are treated as expected generated references. Missing imports elsewhere still produce diagnostics. Build output remains excluded from the graph.

`analysis.diagnostics` includes `parse_error` findings with one-based line/column locations and `unresolved_import` findings for missing relative, configured-alias, or known internal-package imports. External package names, missing bare `baseUrl` imports, and common asset imports are not diagnosed. Already resolved but excluded files are omitted from the graph without a diagnostic. Syntax recovery may leave a partial graph; metrics must be interpreted with the diagnostics.

`oxarch check --strict --json` emits a versioned report containing `analysis`, `metrics`, `violations`, and `check`. The check includes `passed`, `failures`, and the selected thresholds/options. Failure codes are `configuration` (unresolved shared config), `cycles`, `health`, `boundaries`, `parse_errors`, `unresolved_imports` (strict mode), and `no_source_files`. A failed check still emits JSON and exits with status 1. Fatal configuration and I/O errors instead go to stderr and exit nonzero. `--allow-cycles` disables only the cycle check; `--min-health` still applies.

Analyze, check, explorer, and diff JSON have `schema_version: 1`. Consumers should tolerate additional fields. Diff includes diagnostics from both snapshots and `analysis_complete`, so a partial base graph is visible too.

## Actionable reports

Analyze, check, and explorer reports share a `guidance` object: a plain-language summary, reachability scope, boundary-rule coverage, score explanation, entry points, and ordered findings. Each finding has `kind`, `priority`, `title`, `why`, `action`, `files`, and `evidence`. Analysis problems come first, followed by boundary violations, cycle groups, large files, and reachability candidates. Script/test classification uses path conventions and is explicitly tentative.

Cycle evidence lists actual directed imports within the cycle group, not a fabricated cycle order. Large-file findings name the file, measured line count, and 300-line review threshold. Candidate findings explain entry-point limitations and do not prescribe deletion. With no boundary rules, reports say boundaries were not checked. `metrics.score_penalties` exposes the exact cycle, unused-candidate, and large-file deductions; no scoring thresholds or CI pass/fail rules were changed by these explanations.

Text analyze/check output limits the default view to five findings and four files/evidence items per finding, with explicit omission counts. `--details` displays everything, including entry points; JSON never truncates findings. The dashboard offers **Show all findings**, expandable file/evidence lists, and file buttons that focus the graph and module details. Refresh failures label the previous report as potentially stale.

Diff reports include `next_steps` explaining cycle-group changes, import changes, and which affected modules to test. These are review suggestions, not predictions of runtime failure. The CLI shows actual added/removed edges and a bounded affected-module preview; use JSON for the complete lists.

## Incremental cache

Every normal analysis maintains `target/oxarch/parsed-v2.json` inside the analyzed project. Add `target/` to that project's `.gitignore` if needed.

- Cached entries contain source text, extracted imports, parse diagnostics, and line counts.
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
- Current/base diagnostics and an explicit warning when analysis is incomplete.
- Configured entry points and unreachable-module details.
- Dependency-layer and circular layouts with directed arrows.
- Clickable and keyboard-selectable nodes, incoming/outgoing dependency details, and removed connections.
- Search and all/cycle/unused-candidate/changed/affected filters.
- Drag-to-pan, scroll/button zoom, and fit-to-graph controls.
- Added edges in green; removed edges and deleted nodes in dashed pink; cycle borders in amber.
- Branch summaries, changed modules, cycle groups, and boundary explanations.
- **Refresh analysis** to reread sources and recompute the report without restarting the server.

The graph and module table show 100 matching modules per page; previous/next controls expose all results. Edges are drawn only when both endpoints are visible on that page. Module details still list all its current connections. Use search to narrow a large project. Changed/affected views require `--base`.

The explorer analyzes on demand, not automatically on filesystem changes. Filenames, rule messages, and branch labels are rendered as text rather than HTML. API failures are shown in the status area and can be retried with Refresh.

## Current limitations

- Reachability only follows statically discovered imports. Custom framework routes and runtime entry points need explicit configuration; there is no unused-export analysis.
- Module size is measured in lines, not cyclomatic complexity. The health score is an opinionated review signal.
- Dynamic runtime expressions cannot be resolved. Bare `require` calls are detected syntactically, without checking whether that identifier is locally shadowed.
- Vue extraction is not a full single-file-component compiler. Template references, preprocessor semantics, and framework-generated dependencies are not analyzed.
- TypeScript/workspace resolution handles the documented static subset, not every Node/TypeScript resolution mode. Project references, Plug’n’Play, config-package exports maps, custom export-condition sets, and bundler plugins are not implemented.
- Parse diagnostics and recovery do not replace a compiler/type checker. Missing external dependencies and assets are outside this source-graph check.
- The explorer is a local, single-request-at-a-time development server, not a public hosting service. Refresh is manual.
- The graph is paginated for usability; very large graphs may require focused filtering.
