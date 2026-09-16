use crate::{config::ResolverConfig, parser};
use anyhow::{Context, Result};
use petgraph::{algo::kosaraju_scc, graph::NodeIndex, Graph};
use rayon::prelude::*;
use serde::Serialize;
use std::{collections::HashMap, fs, path::{Path, PathBuf}};
use walkdir::WalkDir;

#[derive(Debug, Serialize)]
pub struct AnalysisReport {
    pub files_scanned: usize,
    pub source_files: usize,
    pub dependencies: usize,
    pub cycles: Vec<Vec<String>>,
    pub nodes: Vec<String>,
    pub edges: Vec<DependencyEdge>,
    pub lines: HashMap<String, usize>,
}

#[derive(Debug, Serialize)]
pub struct DependencyEdge { pub from: String, pub to: String }

pub fn analyze(root: &Path) -> Result<AnalysisReport> {
    let root = root.canonicalize().with_context(|| format!("cannot open {}", root.display()))?;
    let config = ResolverConfig::load(&root);
    let mut files = Vec::new();
    let mut files_scanned = 0;
    for entry in WalkDir::new(&root).into_iter().filter_entry(|e| {
        let name = e.file_name().to_string_lossy();
        !matches!(name.as_ref(), "node_modules" | ".git" | "dist" | "build" | ".next" | ".nuxt" | "coverage" | "target")
    }) {
        let entry = entry?;
        if !entry.file_type().is_file() { continue; }
        files_scanned += 1;
        if is_source(entry.path()) { files.push(entry.path().to_path_buf()); }
    }

    let parsed = files.par_iter().map(|file| {
        let content = fs::read_to_string(file).unwrap_or_default();
        (file.clone(), content.lines().count(), parser::imports(file, &content))
    }).collect::<Vec<_>>();

    let mut graph = Graph::<PathBuf, ()>::new();
    let mut indexes: HashMap<PathBuf, NodeIndex> = HashMap::new();
    for file in &files { indexes.insert(file.clone(), graph.add_node(file.clone())); }
    let mut edges = Vec::new();
    let mut lines = HashMap::new();
    for (file, line_count, imports) in parsed {
        lines.insert(relative(&root, &file), line_count);
        for spec in imports {
            if let Some(target) = resolve_import(&root, &file, &spec, &config) {
                if let (Some(&from), Some(&to)) = (indexes.get(&file), indexes.get(&target)) {
                    graph.add_edge(from, to, ());
                    edges.push(DependencyEdge { from: relative(&root, &file), to: relative(&root, &target) });
                }
            }
        }
    }
    let cycles = kosaraju_scc(&graph).into_iter().filter(|g| g.len() > 1).map(|g| g.into_iter().map(|n| relative(&root, &graph[n])).collect()).collect();
    let nodes = files.iter().map(|p| relative(&root, p)).collect();
    Ok(AnalysisReport { files_scanned, source_files: files.len(), dependencies: edges.len(), cycles, nodes, edges, lines })
}

fn relative(root: &Path, path: &Path) -> String { path.strip_prefix(root).unwrap_or(path).display().to_string() }
fn is_source(path: &Path) -> bool { matches!(path.extension().and_then(|e| e.to_str()), Some("ts" | "tsx" | "js" | "jsx" | "vue" | "mjs" | "cjs")) }

fn resolve_import(root: &Path, source: &Path, spec: &str, config: &ResolverConfig) -> Option<PathBuf> {
    if spec.starts_with('.') { return resolve_candidate(&source.parent()?.join(spec)); }
    for (alias, targets) in &config.paths {
        let wildcard = alias.strip_suffix('*');
        let suffix = wildcard.and_then(|prefix| spec.strip_prefix(prefix));
        if suffix.is_none() && alias != spec { continue; }
        for target in targets {
            let mapped = suffix.map_or_else(|| target.clone(), |suffix| target.replace('*', suffix));
            let base = config.base_url.as_deref().map(|p| root.join(p)).unwrap_or_else(|| root.to_path_buf());
            if let Some(found) = resolve_candidate(&base.join(mapped)) { return Some(found); }
        }
    }
    None
}

fn resolve_candidate(base: &Path) -> Option<PathBuf> {
    const EXTS: [&str; 7] = ["ts", "tsx", "js", "jsx", "vue", "mjs", "cjs"];
    if base.is_file() { return base.canonicalize().ok(); }
    for ext in EXTS { let candidate = base.with_extension(ext); if candidate.is_file() { return candidate.canonicalize().ok(); } }
    if base.is_dir() { for ext in EXTS { let candidate = base.join(format!("index.{ext}")); if candidate.is_file() { return candidate.canonicalize().ok(); } } }
    None
}
