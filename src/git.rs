use crate::analyzer::AnalysisReport;
use anyhow::{bail, Context, Result};
use serde::Serialize;
use std::{collections::HashSet, path::Path, process::Command};

#[derive(Debug, Serialize)]
pub struct DiffReport {
    pub base: String,
    pub changed_source_files: Vec<String>,
    pub affected_dependencies: usize,
    pub cycles_in_current_tree: usize,
}

pub fn diff(root: &Path, base: &str, report: &AnalysisReport) -> Result<DiffReport> {
    let range = format!("{base}...HEAD");
    let output = Command::new("git").arg("diff").arg("--name-only").arg(&range).current_dir(root).output().context("failed to run git diff")?;
    if !output.status.success() {
        bail!("git diff failed for base '{base}': {}", String::from_utf8_lossy(&output.stderr));
    }
    let known = report.nodes.iter().map(String::as_str).collect::<HashSet<_>>();
    let mut changed_source_files = String::from_utf8_lossy(&output.stdout).lines().filter(|path| known.contains(*path)).map(str::to_owned).collect::<Vec<_>>();
    changed_source_files.sort();
    let changed = changed_source_files.iter().map(String::as_str).collect::<HashSet<_>>();
    let affected_dependencies = report.edges.iter().filter(|edge| changed.contains(edge.from.as_str()) || changed.contains(edge.to.as_str())).count();
    Ok(DiffReport { base: base.into(), changed_source_files, affected_dependencies, cycles_in_current_tree: report.cycles.len() })
}
