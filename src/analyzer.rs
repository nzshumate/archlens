use crate::{
    cache,
    config::{Configs, ResolverConfig},
    discovery, parser,
    workspace::Workspaces,
};
use anyhow::{Context, Result};
use petgraph::{algo::kosaraju_scc, graph::NodeIndex, Graph};
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
};
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct AnalysisReport {
    pub files_scanned: usize,
    pub source_files: usize,
    pub dependencies: usize,
    pub cycles: Vec<Vec<String>>,
    pub nodes: Vec<String>,
    pub edges: Vec<DependencyEdge>,
    pub lines: HashMap<String, usize>,
    pub diagnostics: Vec<AnalysisDiagnostic>,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct AnalysisDiagnostic {
    pub file: String,
    pub code: String,
    pub message: String,
    pub line: Option<usize>,
    pub column: Option<usize>,
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
    let all_files = discovery::files(root)?;
    let configs = Configs::load(root, &all_files)?;
    let workspaces = Workspaces::discover(&all_files)?;
    let files_scanned = all_files.len();
    let files = all_files
        .into_iter()
        .filter(|path| is_source(path))
        .collect::<Vec<_>>();
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
                .unwrap_or_else(|| {
                    let parsed = parser::parse(f, &source);
                    cache::ParsedSource {
                        lines: source.lines().count(),
                        imports: parsed.imports,
                        issues: parsed.issues,
                        source,
                    }
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
    let mut diagnostics = Vec::new();
    let mut edges = Vec::new();
    let mut lines = HashMap::new();
    for (file, parsed) in parsed {
        let config = configs.for_source(&file);
        let filename = relative(root, &file);
        diagnostics.extend(parsed.issues.iter().map(|issue| AnalysisDiagnostic {
            file: filename.clone(),
            code: "parse_error".into(),
            message: issue.message.clone(),
            line: Some(issue.line),
            column: Some(issue.column),
        }));
        lines.insert(filename.clone(), parsed.lines);
        for spec in &parsed.imports {
            if let Some(target) = resolve_import(root, &file, spec, config, &workspaces) {
                if let (Some(&from), Some(&to)) = (indexes.get(&file), indexes.get(&target)) {
                    graph.add_edge(from, to, ());
                    edges.push(DependencyEdge {
                        from: relative(root, &file),
                        to: relative(root, &target),
                    });
                }
            } else if is_internal(spec, config, &workspaces) && !is_asset(spec) {
                diagnostics.push(AnalysisDiagnostic {
                    file: filename.clone(),
                    code: "unresolved_import".into(),
                    message: format!(
                        "Cannot resolve '{spec}'; check the path, alias, or package entry point"
                    ),
                    line: None,
                    column: None,
                });
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
        diagnostics,
    })
}
fn relative(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}
fn is_source(path: &Path) -> bool {
    matches!(
        path.extension().and_then(|e| e.to_str()),
        Some("ts" | "tsx" | "mts" | "cts" | "js" | "jsx" | "vue" | "mjs" | "cjs")
    )
}
fn resolve_import(
    root: &Path,
    source: &Path,
    spec: &str,
    config: &ResolverConfig,
    workspaces: &Workspaces,
) -> Option<PathBuf> {
    let spec = spec.split('?').next().unwrap_or(spec);
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
        let Some(suffix) = alias_match(alias, spec) else {
            continue;
        };
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
    workspaces
        .candidates(spec)
        .iter()
        .find_map(|path| resolve_candidate(path))
}
fn alias_match<'a>(alias: &str, spec: &'a str) -> Option<Option<&'a str>> {
    if let Some((prefix, suffix)) = alias.split_once('*') {
        Some(Some(spec.strip_prefix(prefix)?.strip_suffix(suffix)?))
    } else {
        (alias == spec).then_some(None)
    }
}
fn is_internal(spec: &str, config: &ResolverConfig, workspaces: &Workspaces) -> bool {
    let spec = spec.split('?').next().unwrap_or(spec);
    spec.starts_with('.')
        || workspaces.contains(spec)
        || config
            .paths
            .keys()
            .any(|alias| alias_match(alias, spec).is_some())
}
fn is_asset(spec: &str) -> bool {
    let path = spec.split('?').next().unwrap_or(spec);
    matches!(
        Path::new(path).extension().and_then(|s| s.to_str()),
        Some(
            "css"
                | "scss"
                | "sass"
                | "less"
                | "styl"
                | "json"
                | "svg"
                | "png"
                | "jpg"
                | "jpeg"
                | "webp"
                | "gif"
                | "ico"
                | "woff"
                | "woff2"
                | "ttf"
                | "mp4"
                | "mp3"
                | "wasm"
                | "html"
        )
    )
}
fn resolve_candidate(base: &Path) -> Option<PathBuf> {
    const EXTS: [&str; 9] = ["ts", "tsx", "mts", "cts", "js", "jsx", "vue", "mjs", "cjs"];
    // Mirror TypeScript's runtime-extension substitutions, without mapping .mjs
    // imports to CommonJS or vice versa.
    let replacements: &[&str] = match base.extension().and_then(|e| e.to_str()) {
        Some("js") => &["ts", "tsx", "js"],
        Some("jsx") => &["tsx", "jsx"],
        Some("mjs") => &["mts", "mjs"],
        Some("cjs") => &["cts", "cjs"],
        _ => &[],
    };
    for ext in replacements {
        let candidate = base.with_extension(ext);
        if candidate.is_file() {
            return candidate.canonicalize().ok();
        }
    }
    if base.is_file() {
        return base.canonicalize().ok();
    }
    for ext in EXTS {
        let mut name = base.as_os_str().to_os_string();
        name.push(format!(".{ext}"));
        let candidate = PathBuf::from(name);
        if candidate.is_file() {
            return candidate.canonicalize().ok();
        }
    }
    if base.is_dir() {
        for ext in EXTS {
            let candidate = base.join(format!("index.{ext}"));
            if candidate.is_file() {
                return candidate.canonicalize().ok();
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
        assert_eq!(resolve_candidate(&emitted), source.canonicalize().ok());
    }

    #[test]
    fn missing_dotted_module_does_not_resolve_to_a_sibling() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("user.ts"), "").unwrap();
        assert_eq!(resolve_candidate(&dir.path().join("user.service")), None);
    }
}
