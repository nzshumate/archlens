use crate::analyzer::{AnalysisReport, DependencyEdge};
use serde::{Deserialize, Serialize};
use std::{fs, path::Path};

#[derive(Debug, Default, Deserialize)]
pub struct RulesConfig {
    #[serde(default)]
    pub boundaries: Vec<BoundaryRule>,
}

#[derive(Debug, Deserialize)]
pub struct BoundaryRule {
    pub from: String,
    pub disallow: String,
    #[serde(default)]
    pub message: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct Violation {
    pub from: String,
    pub to: String,
    pub message: String,
}

pub fn load(root: &Path) -> RulesConfig {
    let path = root.join("archlens.json");
    fs::read_to_string(path)
        .ok()
        .and_then(|raw| serde_json::from_str(&raw).ok())
        .unwrap_or_default()
}

pub fn evaluate(report: &AnalysisReport, config: &RulesConfig) -> Vec<Violation> {
    report.edges.iter().flat_map(|edge| violations_for(edge, config)).collect()
}

fn violations_for(edge: &DependencyEdge, config: &RulesConfig) -> Vec<Violation> {
    config.boundaries.iter().filter(|rule| edge.from.starts_with(&rule.from) && edge.to.starts_with(&rule.disallow)).map(|rule| Violation {
        from: edge.from.clone(),
        to: edge.to.clone(),
        message: rule.message.clone().unwrap_or_else(|| format!("{} may not depend on {}", rule.from, rule.disallow)),
    }).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flags_boundary_violation() {
        let report = AnalysisReport { files_scanned: 2, source_files: 2, dependencies: 1, cycles: vec![], nodes: vec!["ui/a.ts".into(), "data/b.ts".into()], edges: vec![DependencyEdge { from: "ui/a.ts".into(), to: "data/b.ts".into() }] };
        let config = RulesConfig { boundaries: vec![BoundaryRule { from: "ui/".into(), disallow: "data/".into(), message: None }] };
        assert_eq!(evaluate(&report, &config).len(), 1);
    }
}
