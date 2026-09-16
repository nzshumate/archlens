use crate::analyzer::AnalysisReport;
use serde::Serialize;
use std::collections::{HashMap, HashSet};

#[derive(Debug, Serialize)]
pub struct ArchitectureMetrics {
    pub orphan_modules: Vec<String>,
    pub hubs: Vec<ModuleMetric>,
    pub health_score: u8,
}

#[derive(Debug, Serialize)]
pub struct ModuleMetric {
    pub module: String,
    pub fan_in: usize,
    pub fan_out: usize,
}

pub fn calculate(report: &AnalysisReport) -> ArchitectureMetrics {
    let mut incoming: HashMap<&str, usize> = HashMap::new();
    let mut outgoing: HashMap<&str, usize> = HashMap::new();
    for edge in &report.edges {
        *outgoing.entry(&edge.from).or_default() += 1;
        *incoming.entry(&edge.to).or_default() += 1;
    }

    let orphan_modules = report.nodes.iter().filter(|node| {
        incoming.get(node.as_str()).copied().unwrap_or(0) == 0
            && outgoing.get(node.as_str()).copied().unwrap_or(0) == 0
    }).cloned().collect::<Vec<_>>();

    let mut hubs = report.nodes.iter().map(|node| ModuleMetric {
        module: node.clone(),
        fan_in: incoming.get(node.as_str()).copied().unwrap_or(0),
        fan_out: outgoing.get(node.as_str()).copied().unwrap_or(0),
    }).filter(|metric| metric.fan_in >= 5 || metric.fan_out >= 5).collect::<Vec<_>>();
    hubs.sort_by_key(|metric| std::cmp::Reverse(metric.fan_in + metric.fan_out));

    let cycle_nodes = report.cycles.iter().flatten().collect::<HashSet<_>>().len();
    let cycle_penalty = (cycle_nodes.saturating_mul(5)).min(50);
    let orphan_penalty = orphan_modules.len().min(20);
    let health_score = 100usize.saturating_sub(cycle_penalty + orphan_penalty) as u8;

    ArchitectureMetrics { orphan_modules, hubs, health_score }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analyzer::{AnalysisReport, DependencyEdge};

    #[test]
    fn identifies_orphans_and_hubs() {
        let mut nodes = vec!["hub.ts".to_string(), "orphan.ts".to_string()];
        let mut edges = Vec::new();
        for index in 0..5 {
            let leaf = format!("leaf{index}.ts");
            nodes.push(leaf.clone());
            edges.push(DependencyEdge { from: leaf, to: "hub.ts".into() });
        }
        let report = AnalysisReport { files_scanned: nodes.len(), source_files: nodes.len(), dependencies: edges.len(), cycles: vec![], nodes, edges };
        let metrics = calculate(&report);
        assert_eq!(metrics.orphan_modules, vec!["orphan.ts"]);
        assert_eq!(metrics.hubs[0].module, "hub.ts");
        assert_eq!(metrics.hubs[0].fan_in, 5);
    }
}
