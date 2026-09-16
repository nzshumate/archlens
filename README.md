# Oxarch

**See your frontend architecture.**

Oxarch is a native architecture analyzer for TypeScript, JavaScript, React, and Vue projects. Explore module dependencies, find cycles, enforce architectural boundaries, and understand the impact of a branch before merging it.

Written in Rust. Runs locally. No Node.js runtime or cloud service required.

## Features

- **Dependency analysis** — static imports, re-exports, literal dynamic imports, and CommonJS calls, with support for TypeScript aliases and common workspace layouts.
- **Architecture checks** — dependency cycles, heavily connected modules, unused-module candidates, module size, and a health score.
- **Boundary enforcement** — define allowed layers and fail CI when a dependency crosses a prohibited boundary.
- **Git impact reports** — added and removed dependencies, new cycles, deleted modules, and transitively affected importers.
- **Interactive explorer** — searchable dependency graphs, module details, branch overlays, and pan/zoom controls.
- **Incremental parsing** — reuse cached results for unchanged files while resolving dependencies and rebuilding the graph on each run.

## Installation

Build and install from source with a current stable [Rust toolchain](https://www.rust-lang.org/tools/install):

```bash
git clone https://github.com/nzshumate/oxarch.git
cd oxarch
cargo install --path . --locked
```

Ensure Cargo's binary directory is on your `PATH`, then check the installation:

```bash
oxarch --version
```

Git is required for branch comparisons. The explorer's assets are bundled into the binary.

## Quick start

Analyze a frontend project:

```bash
oxarch analyze /path/to/frontend
```

Open its architecture explorer:

```bash
oxarch dev /path/to/frontend
```

Visit [localhost:4242](http://127.0.0.1:4242). Select a module to inspect its imports and importers, search by path, or filter for cycles. Use **Refresh analysis** after editing source files.

## CLI

All commands accept a project path; it defaults to the current directory.

| Command | Purpose |
| --- | --- |
| `oxarch analyze [path]` | Summarize dependencies and architecture health |
| `oxarch analyze [path] --json` | Export the graph, metrics, and violations |
| `oxarch analyze [path] --no-cache` | Analyze without reading or writing the parser cache |
| `oxarch check [path] --min-health 80` | Enforce health, boundaries, and absence of cycles |
| `oxarch check [path] --allow-cycles` | Allow cycles while enforcing health and boundaries |
| `oxarch diff <base> [path] --json` | Report architectural changes since the branch merge base |
| `oxarch dev [path] --base main` | Explore dependencies with branch-impact overlays |
| `oxarch dev [path] --port 4242` | Choose the explorer's local port |

Use `oxarch --help` or `oxarch <command> --help` for command options.

## Enforce architecture boundaries

Create `oxarch.json` in the project being analyzed:

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

Rules match path prefixes. Use a trailing `/` for directory boundaries. See [oxarch.example.json](oxarch.example.json) for a complete example.

Run a local or CI check:

```bash
oxarch check /path/to/frontend --min-health 80
```

The command exits nonzero for boundary violations, cycles, or a health score below the threshold. The default threshold is **70**, with values from 0 to 100 accepted. `--allow-cycles` disables only the cycle check. Invalid rules and analysis errors also produce a nonzero exit status.

## Review branch impact

```bash
oxarch diff main /path/to/frontend
oxarch dev /path/to/frontend --base main
```

Comparisons use the merge base of the selected Git ref and `HEAD`, and include current working-tree changes. Reports show changed and deleted source files, added and removed dependencies, new cycle groups, and affected importing modules.

In the explorer, added edges appear in green; removed edges and deleted modules appear in dashed pink. Cycle modules have amber borders. The graph and module table display 100 matching modules per page; search and pagination provide access to the rest.

## Supported scope

Oxarch scans `.ts`, `.tsx`, `.js`, `.jsx`, `.vue`, `.mjs`, and `.cjs` files. It handles inline Vue scripts, relative and index imports, TypeScript aliases, JSONC configs with relative extensions, and common workspace packages.

- Dead-module detection uses entry-point heuristics. The health score and line-based size metrics are signals for review.
- Resolution does not reproduce every Node.js or TypeScript mode. Package `exports` conditions, project references, package-based tsconfig extensions, and `.mts`/`.cts` sources are not supported.
- Dynamic expressions and external Vue script sources are not resolved. Parse recovery is best effort; use your compiler for syntax and type validation.
- Common generated/vendor directories are skipped; other `.gitignore` patterns are not applied.
- The explorer binds to `127.0.0.1` and is intended for local use. Refresh is manual.

The parser cache lives in `target/oxarch` inside the analyzed project and contains source text. Keep it out of version control. Use `--no-cache` to bypass it or remove that directory to reset it.

See the [analysis reference](docs/reference.md) for resolution details, cache behavior, Git semantics, and limitations.

## Development

See [CONTRIBUTING.md](CONTRIBUTING.md) for development checks, regression testing, and reproducible benchmarks.

## License

Copyright 2026 Nathan Shumate. Licensed under the [Apache License, Version 2.0](LICENSE).
