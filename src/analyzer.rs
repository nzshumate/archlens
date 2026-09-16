use anyhow::{Context, Result};
use petgraph::{algo::kosaraju_scc, graph::NodeIndex, Graph};
use regex::Regex;
use serde::Serialize;
use std::{collections::HashMap, fs, path::{Path, PathBuf}};
use walkdir::WalkDir;

#[derive(Debug, Serialize)]
pub struct AnalysisReport {
    pub files_scanned: usize,
    pub source_files: usize,
    pub dependencies: usize,
    pub cycles: Vec<Vec<String>>,
}

pub fn analyze(root: &Path) -> Result<AnalysisReport> {
    let root = root.canonicalize().with_context(|| format!("cannot open {}", root.display()))?;
    let mut files = Vec::new();
    let mut files_scanned = 0;

    for entry in WalkDir::new(&root).into_iter().filter_entry(|e| {
        let name = e.file_name().to_string_lossy();
        !matches!(name.as_ref(), "node_modules" | ".git" | "dist" | "build" | ".next" | ".nuxt")
    }) {
        let entry = entry?;
        if !entry.file_type().is_file() { continue; }
        files_scanned += 1;
        if is_source(entry.path()) {
            files.push(entry.path().to_path_buf());
        }
    }

    let mut graph = Graph::<PathBuf, ()>::new();
    let mut nodes: HashMap<PathBuf, NodeIndex> = HashMap::new();
    for file in &files {
        nodes.insert(file.clone(), graph.add_node(file.clone()));
    }

    let import_re = Regex::new(r#"(?m)(?:import|export)\s+(?:[^;]*?\s+from\s+)?[\"']([^\"']+)[\"']|require\(\s*[\"']([^\"']+)[\"']\s*\)"#)?;
    let mut dependencies = 0;

    for file in &files {
        let content = fs::read_to_string(file).unwrap_or_default();
        for caps in import_re.captures_iter(&content) {
            let spec = caps.get(1).or_else(|| caps.get(2)).map(|m| m.as_str()).unwrap_or("");
            if !spec.starts_with('.') { continue; }
            if let Some(target) = resolve_import(file, spec) {
                if let (Some(&from), Some(&to)) = (nodes.get(file), nodes.get(&target)) {
                    graph.add_edge(from, to, ());
                    dependencies += 1;
                }
            }
        }
    }

    let cycles = kosaraju_scc(&graph)
        .into_iter()
        .filter(|group| group.len() > 1)
        .map(|group| group.into_iter().map(|n| {
            graph[n].strip_prefix(&root).unwrap_or(&graph[n]).display().to_string()
        }).collect())
        .collect();

    Ok(AnalysisReport { files_scanned, source_files: files.len(), dependencies, cycles })
}

fn is_source(path: &Path) -> bool {
    matches!(path.extension().and_then(|e| e.to_str()), Some("ts" | "tsx" | "js" | "jsx" | "vue" | "mjs" | "cjs"))
}

fn resolve_import(source: &Path, spec: &str) -> Option<PathBuf> {
    let base = source.parent()?.join(spec);
    let extensions = ["ts", "tsx", "js", "jsx", "vue", "mjs", "cjs"];
    if base.is_file() { return base.canonicalize().ok(); }
    for ext in extensions {
        let candidate = base.with_extension(ext);
        if candidate.is_file() { return candidate.canonicalize().ok(); }
    }
    if base.is_dir() {
        for ext in extensions {
            let candidate = base.join(format!("index.{ext}"));
            if candidate.is_file() { return candidate.canonicalize().ok(); }
        }
    }
    None
}
