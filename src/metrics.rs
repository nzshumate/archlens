use crate::analyzer::AnalysisReport;
use serde::Serialize;
use std::collections::{HashMap, HashSet};

#[derive(Debug, Serialize)]
pub struct ArchitectureMetrics {
    pub orphan_modules: Vec<String>,
    pub dead_candidates: Vec<String>,
    pub hubs: Vec<ModuleMetric>,
    pub large_modules: Vec<ModuleComplexity>,
    pub health_score: u8,
}
#[derive(Debug, Serialize)] pub struct ModuleMetric { pub module: String, pub fan_in: usize, pub fan_out: usize }
#[derive(Debug, Serialize)] pub struct ModuleComplexity { pub module: String, pub lines: usize }

pub fn calculate(report: &AnalysisReport) -> ArchitectureMetrics {
    let mut incoming: HashMap<&str, usize> = HashMap::new();
    let mut outgoing: HashMap<&str, usize> = HashMap::new();
    for edge in &report.edges { *outgoing.entry(&edge.from).or_default() += 1; *incoming.entry(&edge.to).or_default() += 1; }
    let orphan_modules = report.nodes.iter().filter(|node| incoming.get(node.as_str()).copied().unwrap_or(0) == 0 && outgoing.get(node.as_str()).copied().unwrap_or(0) == 0).cloned().collect::<Vec<_>>();
    let dead_candidates = report.nodes.iter().filter(|node| incoming.get(node.as_str()).copied().unwrap_or(0) == 0 && !is_entrypoint(node)).cloned().collect::<Vec<_>>();
    let mut hubs = report.nodes.iter().map(|node| ModuleMetric { module: node.clone(), fan_in: incoming.get(node.as_str()).copied().unwrap_or(0), fan_out: outgoing.get(node.as_str()).copied().unwrap_or(0) }).filter(|m| m.fan_in >= 5 || m.fan_out >= 5).collect::<Vec<_>>();
    hubs.sort_by_key(|m| std::cmp::Reverse(m.fan_in + m.fan_out));
    let mut large_modules = report.lines.iter().filter(|(_, lines)| **lines >= 300).map(|(module, lines)| ModuleComplexity { module: module.clone(), lines: *lines }).collect::<Vec<_>>();
    large_modules.sort_by_key(|m| std::cmp::Reverse(m.lines));
    let cycle_nodes = report.cycles.iter().flatten().collect::<HashSet<_>>().len();
    let penalty = (cycle_nodes * 5).min(45) + dead_candidates.len().min(20) + large_modules.len().min(15);
    let health_score = 100usize.saturating_sub(penalty) as u8;
    ArchitectureMetrics { orphan_modules, dead_candidates, hubs, large_modules, health_score }
}

fn is_entrypoint(path: &str) -> bool {
    let name = path.rsplit('/').next().unwrap_or(path);
    name.starts_with("main.") || name.starts_with("index.") || name.starts_with("app.") || name.starts_with("App.")
}

#[cfg(test)] mod tests {
    use super::*; use crate::analyzer::{AnalysisReport, DependencyEdge};
    #[test] fn identifies_orphans_and_hubs() {
        let mut nodes = vec!["hub.ts".to_string(), "orphan.ts".to_string()]; let mut edges = Vec::new();
        for index in 0..5 { let leaf = format!("leaf{index}.ts"); nodes.push(leaf.clone()); edges.push(DependencyEdge { from: leaf, to: "hub.ts".into() }); }
        let report = AnalysisReport { files_scanned: nodes.len(), source_files: nodes.len(), dependencies: edges.len(), cycles: vec![], nodes, edges, lines: HashMap::new() };
        let metrics = calculate(&report); assert_eq!(metrics.orphan_modules, vec!["orphan.ts"]); assert_eq!(metrics.hubs[0].module, "hub.ts"); assert_eq!(metrics.hubs[0].fan_in, 5);
    }
}
