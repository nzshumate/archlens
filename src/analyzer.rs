use crate::{cache, config::ResolverConfig, parser, workspace::Workspaces};
use anyhow::{Context, Result};
use petgraph::{algo::kosaraju_scc, graph::NodeIndex, Graph};
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
};
use walkdir::WalkDir;
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct AnalysisReport {
    pub files_scanned: usize,
    pub source_files: usize,
    pub dependencies: usize,
    pub cycles: Vec<Vec<String>>,
    pub nodes: Vec<String>,
    pub edges: Vec<DependencyEdge>,
    pub lines: HashMap<String, usize>,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct DependencyEdge {
    pub from: String,
    pub to: String,
}
pub fn analyze(root: &Path) -> Result<AnalysisReport> {
    let root = root
        .canonicalize()
        .with_context(|| format!("cannot open {}", root.display()))?;
    analyze_with_cache(&root, true)
}
pub fn analyze_with_cache(root: &Path, use_cache: bool) -> Result<AnalysisReport> {
    let canonical = root.canonicalize()?;
    let root = canonical.as_path();
    let mut cache = if use_cache {
        cache::Cache::load(root)
    } else {
        cache::Cache::default()
    };
    let config = ResolverConfig::load(root);
    let workspaces = Workspaces::discover(root);
    let mut files = Vec::new();
    let mut files_scanned = 0;
    for entry in WalkDir::new(root).into_iter().filter_entry(|e| {
        let n = e.file_name().to_string_lossy();
        !matches!(
            n.as_ref(),
            "node_modules" | ".git" | "dist" | "build" | ".next" | ".nuxt" | "coverage" | "target"
        )
    }) {
        let entry = entry?;
        if !entry.file_type().is_file() {
            continue;
        }
        files_scanned += 1;
        if is_source(entry.path()) {
            files.push(entry.path().to_path_buf());
        }
    }
    files.sort();
    let parsed = files
        .par_iter()
        .map(|f| {
            let source =
                fs::read_to_string(f).with_context(|| format!("cannot read {}", f.display()))?;
            let key = relative(root, f);
            let parsed = cache
                .entries
                .get(&key)
                .filter(|entry| entry.source == source)
                .cloned()
                .unwrap_or_else(|| cache::ParsedSource {
                    lines: source.lines().count(),
                    imports: parser::imports(f, &source),
                    source,
                });
            Ok((f.clone(), parsed))
        })
        .collect::<Result<Vec<_>>>()?;
    let cache_unchanged = parsed.len() == cache.entries.len()
        && parsed.iter().all(|(file, parsed)| {
            cache
                .entries
                .get(&relative(root, file))
                .is_some_and(|old| old.source == parsed.source)
        });
    cache.entries.clear();
    let mut graph = Graph::<PathBuf, ()>::new();
    let mut indexes = HashMap::<PathBuf, NodeIndex>::new();
    for f in &files {
        indexes.insert(f.clone(), graph.add_node(f.clone()));
    }
    let mut edges = Vec::new();
    let mut lines = HashMap::new();
    for (file, parsed) in parsed {
        lines.insert(relative(root, &file), parsed.lines);
        for spec in &parsed.imports {
            if let Some(target) = resolve_import(root, &file, spec, &config, &workspaces) {
                if let (Some(&from), Some(&to)) = (indexes.get(&file), indexes.get(&target)) {
                    graph.add_edge(from, to, ());
                    edges.push(DependencyEdge {
                        from: relative(root, &file),
                        to: relative(root, &target),
                    });
                }
            }
        }
        cache.entries.insert(relative(root, &file), parsed);
    }
    if use_cache && !cache_unchanged {
        cache.save(root);
    }
    edges.sort_by(|a, b| (&a.from, &a.to).cmp(&(&b.from, &b.to)));
    edges.dedup_by(|a, b| a.from == b.from && a.to == b.to);
    let mut cycles: Vec<Vec<String>> = kosaraju_scc(&graph)
        .into_iter()
        .filter(|g| g.len() > 1 || graph.contains_edge(g[0], g[0]))
        .map(|g| g.into_iter().map(|n| relative(root, &graph[n])).collect())
        .collect();
    for cycle in &mut cycles {
        cycle.sort();
    }
    cycles.sort();
    let nodes = files.iter().map(|p| relative(root, p)).collect();
    Ok(AnalysisReport {
        files_scanned,
        source_files: files.len(),
        dependencies: edges.len(),
        cycles,
        nodes,
        edges,
        lines,
    })
}
fn relative(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .display()
        .to_string()
}
fn is_source(path: &Path) -> bool {
    matches!(
        path.extension().and_then(|e| e.to_str()),
        Some("ts" | "tsx" | "js" | "jsx" | "vue" | "mjs" | "cjs")
    )
}
fn resolve_import(
    root: &Path,
    source: &Path,
    spec: &str,
    config: &ResolverConfig,
    workspaces: &Workspaces,
) -> Option<PathBuf> {
    if spec.starts_with('.') {
        return resolve_candidate(&source.parent()?.join(spec));
    }
    let mut aliases = config.paths.iter().collect::<Vec<_>>();
    aliases.sort_by(|(a, _), (b, _)| {
        let score = |alias: &str| {
            (
                usize::from(!alias.contains('*')),
                alias.find('*').unwrap_or(alias.len()),
            )
        };
        score(b).cmp(&score(a)).then_with(|| a.cmp(b))
    });
    for (alias, targets) in aliases {
        let wildcard = alias.strip_suffix('*');
        let suffix = wildcard.and_then(|prefix| spec.strip_prefix(prefix));
        if suffix.is_none() && alias != spec {
            continue;
        }
        for target in targets {
            let mapped = suffix.map_or_else(|| target.clone(), |s| target.replace('*', s));
            let base = config
                .base_url
                .as_deref()
                .or(config.paths_base.as_deref())
                .map(|p| root.join(p))
                .unwrap_or_else(|| root.to_path_buf());
            if let Some(found) = resolve_candidate(&base.join(mapped)) {
                return Some(found);
            }
        }
    }
    if let Some(base) = &config.base_url {
        if let Some(found) = resolve_candidate(&root.join(base).join(spec)) {
            return Some(found);
        }
    }
    workspaces.resolve(spec).and_then(|p| resolve_candidate(&p))
}
fn resolve_candidate(base: &Path) -> Option<PathBuf> {
    const EXTS: [&str; 7] = ["ts", "tsx", "js", "jsx", "vue", "mjs", "cjs"];
    if base.is_file() {
        return base.canonicalize().ok();
    }
    // Preserve dotted module names when probing extensionless imports.
    for ext in EXTS {
        let mut name = base.as_os_str().to_os_string();
        name.push(format!(".{ext}"));
        let candidate = PathBuf::from(name);
        if candidate.is_file() {
            return candidate.canonicalize().ok();
        }
    }
    // TypeScript projects may refer to the emitted JavaScript filename.
    if matches!(
        base.extension().and_then(|ext| ext.to_str()),
        Some("js" | "jsx" | "mjs" | "cjs")
    ) {
        for ext in EXTS {
            let candidate = base.with_extension(ext);
            if candidate.is_file() {
                return candidate.canonicalize().ok();
            }
        }
    }
    if base.is_dir() {
        for ext in EXTS {
            let c = base.join(format!("index.{ext}"));
            if c.is_file() {
                return c.canonicalize().ok();
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::resolve_candidate;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn resolves_dotted_modules_and_directories() {
        let dir = tempdir().unwrap();
        for extension in ["ts", "tsx", "js", "jsx", "vue", "mjs", "cjs"] {
            let name = format!("module.{extension}");
            let target = dir.path().join(format!("{name}.{extension}"));
            fs::write(&target, "").unwrap();
            assert_eq!(
                resolve_candidate(&dir.path().join(name)),
                target.canonicalize().ok()
            );
        }
        let directory = dir.path().join("feature.module");
        fs::create_dir(&directory).unwrap();
        let index = directory.join("index.ts");
        fs::write(&index, "").unwrap();
        assert_eq!(resolve_candidate(&directory), index.canonicalize().ok());
    }

    #[test]
    fn preserves_explicit_files_and_emitted_javascript_imports() {
        let dir = tempdir().unwrap();
        let source = dir.path().join("user.service.ts");
        fs::write(&source, "").unwrap();
        assert_eq!(resolve_candidate(&source), source.canonicalize().ok());
        let emitted = dir.path().join("user.service.js");
        assert_eq!(resolve_candidate(&emitted), source.canonicalize().ok());
        fs::write(&emitted, "").unwrap();
        assert_eq!(resolve_candidate(&emitted), emitted.canonicalize().ok());
    }

    #[test]
    fn missing_dotted_module_does_not_resolve_to_a_sibling() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("user.ts"), "").unwrap();
        assert_eq!(resolve_candidate(&dir.path().join("user.service")), None);
    }
}
