use crate::analyzer::{AnalysisReport, DependencyEdge};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::{fs, path::Path};

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RulesConfig {
    #[serde(default)]
    pub boundaries: Vec<BoundaryRule>,
    #[serde(default, rename = "entryPoints")]
    pub entry_points: Vec<String>,
}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
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

pub fn load(root: &Path) -> Result<RulesConfig> {
    let path = root.join("oxarch.json");
    let raw = match fs::read_to_string(&path) {
        Ok(raw) => raw,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(RulesConfig::default())
        }
        Err(error) => return Err(error).with_context(|| format!("cannot read {}", path.display())),
    };
    let config: RulesConfig = serde_json::from_str(&raw)
        .with_context(|| format!("invalid architecture rules in {}", path.display()))?;
    for entry in &config.entry_points {
        anyhow::ensure!(
            !entry.is_empty()
                && !Path::new(entry).is_absolute()
                && !entry.contains('\\')
                && !entry.split('/').any(|part| part == ".."),
            "entryPoints must be project-relative paths using forward slashes: {entry}"
        );
    }
    for boundary in &config.boundaries {
        anyhow::ensure!(
            !boundary.from.is_empty() && !boundary.disallow.is_empty(),
            "boundary from/disallow prefixes cannot be empty"
        );
    }
    Ok(config)
}

pub fn evaluate(report: &AnalysisReport, config: &RulesConfig) -> Vec<Violation> {
    report
        .edges
        .iter()
        .flat_map(|edge| violations_for(edge, config))
        .collect()
}
fn violations_for(edge: &DependencyEdge, config: &RulesConfig) -> Vec<Violation> {
    config
        .boundaries
        .iter()
        .filter(|rule| edge.from.starts_with(&rule.from) && edge.to.starts_with(&rule.disallow))
        .map(|rule| Violation {
            from: edge.from.clone(),
            to: edge.to.clone(),
            message: rule
                .message
                .clone()
                .unwrap_or_else(|| format!("{} may not depend on {}", rule.from, rule.disallow)),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    #[test]
    fn flags_boundary_violation() {
        let report = AnalysisReport {
            files_scanned: 2,
            source_files: 2,
            dependencies: 1,
            cycles: vec![],
            nodes: vec!["ui/a.ts".into(), "data/b.ts".into()],
            edges: vec![DependencyEdge {
                from: "ui/a.ts".into(),
                to: "data/b.ts".into(),
            }],
            lines: HashMap::new(),
            ..AnalysisReport::default()
        };
        let config = RulesConfig {
            boundaries: vec![BoundaryRule {
                from: "ui/".into(),
                disallow: "data/".into(),
                message: None,
            }],
            ..RulesConfig::default()
        };
        assert_eq!(evaluate(&report, &config).len(), 1);
    }
}
