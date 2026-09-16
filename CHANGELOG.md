# Changelog

## Unreleased

### Added

- Installed-package TypeScript configuration extensions, including hoisted/scoped configs and multiple bases.
- Next.js/Expo entry-point detection and framework-aware reachability.
- Narrow handling of expected generated Next.js type references.
- Partial analysis with actionable diagnostics when shared configuration packages are missing; CI checks still fail.

- Project-local `.gitignore` and `.oxarchignore` support.
- Explicit `entryPoints` for module reachability, including disconnected cycles.
- Parse diagnostics with locations and unresolved internal-import diagnostics.
- `check --json` with structured failure reasons and `--strict` for unresolved imports.
- Nearest-package TypeScript/JavaScript configuration, modern TypeScript extensions, and source-oriented workspace exports.
- External Vue scripts and separate parsing of inline script blocks.
- Explorer diagnostics, incomplete-analysis warnings, and an unused-candidate filter.
- Versioned JSON reports and diagnostics from both Git comparison snapshots.

### Behavior changes

- CI checks fail on parse errors, missing shared configuration packages, and empty source graphs.
- Declaration files are no longer classified as unused-module candidates.
- Invalid, missing, or circular relative configuration extensions fail explicitly. Missing installed-package extensions produce diagnostics and fail CI checks.
- Unknown architecture-rule fields, invalid entry points, malformed package manifests, duplicate internal package names, and invalid ignore patterns fail explicitly.
- Ignored files are excluded even if tracked by Git. Parent/global ignore settings are not used.
- The parser cache uses `parsed-v2.json` to preserve parse diagnostics. Older cache files are ignored and can be removed.

The release version remains 0.2.0 until a release is prepared. Nothing in this section implies publication to a package registry.
