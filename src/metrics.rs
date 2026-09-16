use crate::{analyzer::AnalysisReport, rules::RulesConfig};
use anyhow::{ensure, Result};
use serde::Serialize;
use std::collections::{HashMap, HashSet};

#[derive(Debug, Serialize)]
pub struct ArchitectureMetrics {
    pub orphan_modules: Vec<String>,
    pub dead_candidates: Vec<String>,
    pub hubs: Vec<ModuleMetric>,
    pub large_modules: Vec<ModuleComplexity>,
    pub health_score: u8,
    pub entry_points: Vec<String>,
    pub reachability_mode: String,
}
#[derive(Debug, Serialize)]
pub struct ModuleMetric {
    pub module: String,
    pub fan_in: usize,
    pub fan_out: usize,
}
#[derive(Debug, Serialize)]
pub struct ModuleComplexity {
    pub module: String,
    pub lines: usize,
}

pub fn calculate(report: &AnalysisReport, config: &RulesConfig) -> Result<ArchitectureMetrics> {
    let mut incoming: HashMap<&str, usize> = HashMap::new();
    let mut outgoing: HashMap<&str, usize> = HashMap::new();
    for edge in &report.edges {
        *outgoing.entry(&edge.from).or_default() += 1;
        *incoming.entry(&edge.to).or_default() += 1;
    }
    let orphan_modules = report
        .nodes
        .iter()
        .filter(|node| {
            incoming.get(node.as_str()).copied().unwrap_or(0) == 0
                && outgoing.get(node.as_str()).copied().unwrap_or(0) == 0
        })
        .cloned()
        .collect::<Vec<_>>();
    let mut entry_points = config
        .entry_points
        .iter()
        .map(|path| path.trim_start_matches("./").to_owned())
        .collect::<Vec<_>>();
    entry_points.sort();
    entry_points.dedup();
    for entry in &entry_points {
        ensure!(
            report.nodes.contains(entry),
            "entry point '{entry}' is missing, ignored, or not a supported source file"
        );
    }
    let explicit = !entry_points.is_empty();
    let dead_candidates = if explicit {
        let mut adjacency: HashMap<&str, Vec<&str>> = HashMap::new();
        for edge in &report.edges {
            adjacency.entry(&edge.from).or_default().push(&edge.to);
        }
        let mut pending = entry_points.iter().map(String::as_str).collect::<Vec<_>>();
        let mut reached = HashSet::new();
        while let Some(node) = pending.pop() {
            if reached.insert(node) {
                pending.extend(adjacency.get(node).into_iter().flatten().copied());
            }
        }
        report
            .nodes
            .iter()
            .filter(|node| !reached.contains(node.as_str()))
            .cloned()
            .collect::<Vec<_>>()
    } else {
        report
            .nodes
            .iter()
            .filter(|node| {
                incoming.get(node.as_str()).copied().unwrap_or(0) == 0 && !is_entrypoint(node)
            })
            .cloned()
            .collect::<Vec<_>>()
    };
    let mut hubs = report
        .nodes
        .iter()
        .map(|node| ModuleMetric {
            module: node.clone(),
            fan_in: incoming.get(node.as_str()).copied().unwrap_or(0),
            fan_out: outgoing.get(node.as_str()).copied().unwrap_or(0),
        })
        .filter(|m| m.fan_in >= 5 || m.fan_out >= 5)
        .collect::<Vec<_>>();
    hubs.sort_by_key(|m| std::cmp::Reverse(m.fan_in + m.fan_out));
    let mut large_modules = report
        .lines
        .iter()
        .filter(|(_, lines)| **lines >= 300)
        .map(|(module, lines)| ModuleComplexity {
            module: module.clone(),
            lines: *lines,
        })
        .collect::<Vec<_>>();
    large_modules.sort_by(|a, b| b.lines.cmp(&a.lines).then_with(|| a.module.cmp(&b.module)));
    let cycle_nodes = report.cycles.iter().flatten().collect::<HashSet<_>>().len();
    let penalty =
        (cycle_nodes * 5).min(45) + dead_candidates.len().min(20) + large_modules.len().min(15);
    let health_score = 100usize.saturating_sub(penalty) as u8;
    Ok(ArchitectureMetrics {
        orphan_modules,
        dead_candidates,
        hubs,
        large_modules,
        health_score,
        entry_points,
        reachability_mode: if explicit { "explicit" } else { "heuristic" }.into(),
    })
}

fn is_entrypoint(path: &str) -> bool {
    let name = path.rsplit('/').next().unwrap_or(path);
    name.starts_with("main.")
        || name.starts_with("index.")
        || name.starts_with("app.")
        || name.starts_with("App.")
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
            edges.push(DependencyEdge {
                from: leaf,
                to: "hub.ts".into(),
            });
        }
        let report = AnalysisReport {
            files_scanned: nodes.len(),
            source_files: nodes.len(),
            dependencies: edges.len(),
            cycles: vec![],
            nodes,
            edges,
            lines: HashMap::new(),
            ..AnalysisReport::default()
        };
        let metrics = calculate(&report, &RulesConfig::default()).unwrap();
        assert_eq!(metrics.orphan_modules, vec!["orphan.ts"]);
        assert_eq!(metrics.hubs[0].module, "hub.ts");
        assert_eq!(metrics.hubs[0].fan_in, 5);
    }
}
