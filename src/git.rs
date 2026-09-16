use crate::analyzer::{self, AnalysisDiagnostic, AnalysisReport, DependencyEdge};
use anyhow::{bail, Context, Result};
use serde::Serialize;
use std::{
    collections::{BTreeSet, HashSet},
    fs,
    path::{Path, PathBuf},
    process::Command,
};

#[derive(Debug, Serialize)]
pub struct DiffReport {
    pub schema_version: u32,
    pub base: String,
    pub analysis_complete: bool,
    pub base_diagnostics: Vec<AnalysisDiagnostic>,
    pub current_diagnostics: Vec<AnalysisDiagnostic>,
    pub merge_base: String,
    pub changed_source_files: Vec<String>,
    pub deleted_source_files: Vec<String>,
    pub affected_modules: Vec<String>,
    pub affected_dependencies: usize,
    pub added_dependencies: Vec<String>,
    pub removed_dependencies: Vec<String>,
    pub added_edges: Vec<DependencyEdge>,
    pub removed_edges: Vec<DependencyEdge>,
    pub new_cycles: Vec<Vec<String>>,
    pub cycles_in_current_tree: usize,
}
fn git(root: &Path, args: &[&str]) -> Result<String> {
    let output = Command::new("git")
        .args(args)
        .current_dir(root)
        .output()
        .context("failed to run git")?;
    if !output.status.success() {
        bail!("git failed: {}", String::from_utf8_lossy(&output.stderr));
    }
    Ok(String::from_utf8(output.stdout)?.trim().to_owned())
}
struct Snapshot {
    repository: PathBuf,
    tree: PathBuf,
    _temp: tempfile::TempDir,
}
impl Drop for Snapshot {
    fn drop(&mut self) {
        let _ = Command::new("git")
            .args(["worktree", "remove", "--force"])
            .arg(&self.tree)
            .current_dir(&self.repository)
            .output();
    }
}
pub fn diff(root: &Path, base: &str, report: &AnalysisReport) -> Result<DiffReport> {
    let root = root.canonicalize()?;
    let repository =
        PathBuf::from(git(&root, &["rev-parse", "--show-toplevel"])?).canonicalize()?;
    let prefix = root.strip_prefix(&repository)?;
    let reference = git(
        &repository,
        &[
            "rev-parse",
            "--verify",
            "--end-of-options",
            &format!("{base}^{{commit}}"),
        ],
    )?;
    let merge_base = git(&repository, &["merge-base", &reference, "HEAD"])?;
    let temp = tempfile::tempdir()?;
    let tree = temp.path().join("tree");
    let output = Command::new("git")
        .args(["worktree", "add", "--detach"])
        .arg(&tree)
        .arg(&merge_base)
        .current_dir(&repository)
        .output()?;
    if !output.status.success() {
        bail!(
            "cannot create base snapshot: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    let snapshot = Snapshot {
        repository: repository.clone(),
        tree,
        _temp: temp,
    };
    let base_root = snapshot.tree.join(prefix);
    // A newly created frontend directory has an empty base graph.
    let empty = tempfile::tempdir()?;
    let base_report = analyzer::analyze_with_cache(
        if base_root.exists() {
            &base_root
        } else {
            empty.path()
        },
        false,
    )?;
    let all_nodes = report
        .nodes
        .iter()
        .chain(&base_report.nodes)
        .cloned()
        .collect::<BTreeSet<_>>();
    let mut changed_source_files = Vec::new();
    for node in all_nodes {
        let read = |path: PathBuf| -> Result<Option<Vec<u8>>> {
            match fs::read(path) {
                Ok(bytes) => Ok(Some(bytes)),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
                Err(error) => Err(error.into()),
            }
        };
        if read(root.join(&node))? != read(base_root.join(&node))? {
            changed_source_files.push(node);
        }
    }
    let current_nodes = report.nodes.iter().collect::<HashSet<_>>();
    let deleted_source_files = base_report
        .nodes
        .iter()
        .filter(|n| !current_nodes.contains(n) && !root.join(n).exists())
        .cloned()
        .collect();
    let edge_set = |r: &AnalysisReport| {
        r.edges
            .iter()
            .map(|e| (e.from.clone(), e.to.clone()))
            .collect::<BTreeSet<_>>()
    };
    let current = edge_set(report);
    let previous = edge_set(&base_report);
    let edges = |set: Vec<&(String, String)>| {
        set.into_iter()
            .map(|(from, to)| DependencyEdge {
                from: from.clone(),
                to: to.clone(),
            })
            .collect::<Vec<_>>()
    };
    let added_edges = edges(current.difference(&previous).collect());
    let removed_edges = edges(previous.difference(&current).collect());
    let labels = |edges: &[DependencyEdge]| {
        edges
            .iter()
            .map(|e| format!("{} -> {}", e.from, e.to))
            .collect()
    };
    let mut affected = changed_source_files
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    // Resolution-only changes (tsconfig/package edits) also affect both endpoints.
    for edge in added_edges.iter().chain(&removed_edges) {
        affected.insert(edge.from.clone());
        affected.insert(edge.to.clone());
    }
    loop {
        let before = affected.len();
        for (from, to) in current.union(&previous) {
            if affected.contains(to) {
                affected.insert(from.clone());
            }
        }
        if affected.len() == before {
            break;
        }
    }
    let affected_dependencies = current
        .union(&previous)
        .filter(|(from, to)| affected.contains(from) || affected.contains(to))
        .count();
    let base_cycles = base_report.cycles.iter().cloned().collect::<HashSet<_>>();
    let new_cycles = report
        .cycles
        .iter()
        .filter(|c| !base_cycles.contains(*c))
        .cloned()
        .collect();
    Ok(DiffReport {
        schema_version: 1,
        analysis_complete: report.diagnostics.is_empty() && base_report.diagnostics.is_empty(),
        base_diagnostics: base_report.diagnostics.clone(),
        current_diagnostics: report.diagnostics.clone(),
        base: base.into(),
        merge_base,
        changed_source_files,
        deleted_source_files,
        affected_modules: affected.into_iter().collect(),
        affected_dependencies,
        added_dependencies: labels(&added_edges),
        removed_dependencies: labels(&removed_edges),
        added_edges,
        removed_edges,
        new_cycles,
        cycles_in_current_tree: report.cycles.len(),
    })
}
