use crate::{
    analyzer::AnalysisReport,
    metrics::ArchitectureMetrics,
    rules::{RulesConfig, Violation},
};
use serde::Serialize;
use std::collections::HashSet;

#[derive(Debug, Serialize)]
pub struct Guidance {
    pub summary: String,
    pub scope: String,
    pub boundaries: String,
    pub boundary_rule_count: usize,
    pub score_explanation: String,
    pub findings: Vec<Finding>,
    pub entry_points: Vec<String>,
}
#[derive(Debug, Serialize)]
pub struct Finding {
    pub kind: String,
    pub priority: String,
    pub title: String,
    pub why: String,
    pub action: String,
    pub files: Vec<String>,
    pub evidence: Vec<String>,
}
fn finding(
    kind: &str,
    priority: &str,
    title: String,
    why: &str,
    action: &str,
    files: Vec<String>,
    evidence: Vec<String>,
) -> Finding {
    Finding {
        kind: kind.into(),
        priority: priority.into(),
        title,
        why: why.into(),
        action: action.into(),
        files,
        evidence,
    }
}
pub fn build(
    a: &AnalysisReport,
    m: &ArchitectureMetrics,
    rules: &RulesConfig,
    violations: &[Violation],
) -> Guidance {
    let mut findings = Vec::new();
    if a.source_files == 0 {
        findings.push(finding("empty", "fix first", "No supported source files found".into(), "There is no source graph to evaluate.", "Check the analysis directory and ignore rules. Oxarch scans JavaScript, TypeScript and Vue, not Rust or other backend languages.", vec![], vec![]));
    }
    for diagnostic in &a.diagnostics {
        let (title, action) = match diagnostic.code.as_str() {
            "unresolved_config" => ("Restore shared TypeScript configuration", "Install dependencies in the package containing this config, or correct its extends value. Rerun Oxarch before trusting reachability or the score."),
            "parse_error" => ("Fix a source parsing error", "Open the reported location and run your project's compiler or type checker. Fix invalid syntax, or verify parser support if the source is valid; rerun analysis."),
            _ => ("Resolve an internal import", "Check the import spelling, file casing, path aliases and package exports. If a build generates this file, generate it first and rerun analysis."),
        };
        let location = diagnostic.line.map_or_else(
            || diagnostic.file.clone(),
            |line| {
                format!(
                    "{}:{line}:{}",
                    diagnostic.file,
                    diagnostic.column.unwrap_or(1)
                )
            },
        );
        findings.push(finding(&diagnostic.code, "fix first", title.into(), "This finding can leave dependencies out of the graph, making later conclusions incomplete.", action, vec![diagnostic.file.clone()], vec![format!("{location}: {}", diagnostic.message)]));
    }
    for violation in violations {
        findings.push(finding("boundary", "fix", "A dependency crosses a configured boundary".into(), &violation.message, "Move the dependency behind an allowed service/interface, or revise the boundary rule if the architecture intentionally changed.", vec![violation.from.clone(), violation.to.clone()], vec![format!("{} imports {}", violation.from, violation.to)]));
    }
    for cycle in &a.cycles {
        let members = cycle.iter().collect::<HashSet<_>>();
        let evidence = a
            .edges
            .iter()
            .filter(|e| members.contains(&e.from) && members.contains(&e.to))
            .map(|e| format!("{} imports {}", e.from, e.to))
            .collect();
        findings.push(finding("cycle", "review", format!("Untangle a cycle involving {} module(s)", cycle.len()), "These modules depend on one another, directly or indirectly. That can make changes and initialization harder to reason about; type-only imports also count.", "Review the actual imports in the evidence. Move shared types or helpers into a lower-level module, or pass a dependency in. Rerun analysis to confirm the cycle is gone.", cycle.clone(), evidence));
    }
    for module in &m.large_modules {
        findings.push(finding("large_module", "review", format!("Review responsibilities in {}", module.module), "This file exceeds the 300-line review threshold. Length alone does not establish a defect or excessive complexity.", "Look for independent responsibilities that can be extracted and tested separately. Keep cohesive code together; do not split a file just to improve the score.", vec![module.module.clone()], vec![format!("{} lines (threshold: 300)", module.lines)]));
    }
    let (tooling, candidates): (Vec<_>, Vec<_>) = m
        .dead_candidates
        .iter()
        .cloned()
        .partition(|p| is_tooling(p));
    for (kind, files) in [("reachability", candidates), ("tooling", tooling)] {
        if files.is_empty() {
            continue;
        }
        let title = if kind == "tooling" {
            format!(
                "Confirm {} possible test/script entry point(s)",
                files.len()
            )
        } else {
            format!("Verify {} possibly unused module(s)", files.len())
        };
        let why = if m.reachability_mode == "heuristic" {
            "These files have no incoming imports and do not match the filename entry-point heuristic. This does not prove they are unused."
        } else {
            "No import path reaches these files from the analysis entry points. Scripts, tests and dynamically loaded modules can still be intentional."
        };
        let action = if kind == "tooling" {
            "Check package scripts and the test runner. If these are intentional roots, add them alongside existing roots in oxarch.json entryPoints. Explicit entryPoints replaces automatic detection. Do not delete files based on this list."
        } else {
            "Check runtime loading and framework conventions. Add intentional entry files alongside existing roots in oxarch.json entryPoints; only consider removal after verifying the files are unused."
        };
        findings.push(finding(kind, "verify", title, why, action, files, vec![]));
    }
    let summary = if a.source_files == 0 {
        "Analysis needs setup: no supported source files were found.".into()
    } else if !a.diagnostics.is_empty() {
        format!(
            "Fix {} analysis issue(s) first. The graph and score may be incomplete.",
            a.diagnostics.len()
        )
    } else if !violations.is_empty() || !a.cycles.is_empty() {
        format!("Review {} cycle group(s) and {} boundary violation(s), then the maintenance candidates below.", a.cycles.len(), violations.len())
    } else if !findings.is_empty() {
        "No cycles or configured-boundary violations found. The items below are review candidates, not confirmed defects.".into()
    } else {
        "No findings under the current checks. This does not replace tests or type checking.".into()
    };
    let scope = if m.reachability_mode == "heuristic" {
        "Reachability uses filename heuristics. Configure entryPoints for a graph-based check of your actual entry files.".into()
    } else {
        format!("Reachability follows imports from {} {} entry point(s). Tests, scripts and runtime-loaded files may need additional roots.", m.entry_points.len(), if m.reachability_mode == "framework" { "automatically detected framework" } else { "configured" })
    };
    let boundaries = if rules.boundaries.is_empty() {
        "Boundaries not checked: no rules configured in oxarch.json.".into()
    } else {
        format!(
            "{} boundary rule(s) checked; {} violation(s).",
            rules.boundaries.len(),
            violations.len()
        )
    };
    let p = &m.score_penalties;
    let score_explanation = format!("Score: 100 - {} for cycle members - {} for reachability candidates - {} for large files = {}/100. This is a review signal, not a production-readiness grade.{}", p.cycles, p.unused_candidates, p.large_modules, m.health_score, if a.diagnostics.is_empty() { "" } else { " Analysis is incomplete." });
    Guidance {
        summary,
        scope,
        boundaries,
        boundary_rule_count: rules.boundaries.len(),
        score_explanation,
        findings,
        entry_points: m.entry_points.clone(),
    }
}
fn is_tooling(path: &str) -> bool {
    path.split('/')
        .any(|part| matches!(part, "scripts" | "tests" | "__tests__" | "test"))
        || path.contains(".test.")
        || path.contains(".spec.")
}
pub fn print(g: &Guidance, details: bool) {
    println!("\n{}\n{}\n{}", g.summary, g.scope, g.boundaries);
    println!("{}", g.score_explanation);
    if details && !g.entry_points.is_empty() {
        println!("Entry points used:");
        for entry in &g.entry_points {
            println!("  {entry}");
        }
    }
    let limit = if details { usize::MAX } else { 5 };
    for (index, finding) in g.findings.iter().take(limit).enumerate() {
        println!("\n{}. [{}] {}", index + 1, finding.priority, finding.title);
        println!("   Why: {}", finding.why);
        for file in finding
            .files
            .iter()
            .take(if details { usize::MAX } else { 4 })
        {
            println!("   File: {file}");
        }
        if !details && finding.files.len() > 4 {
            println!(
                "   ... {} more files; use --details to see all.",
                finding.files.len() - 4
            );
        }
        for evidence in finding
            .evidence
            .iter()
            .take(if details { usize::MAX } else { 4 })
        {
            println!("   Evidence: {evidence}");
        }
        if !details && finding.evidence.len() > 4 {
            println!(
                "   ... {} more evidence items; use --details to see all.",
                finding.evidence.len() - 4
            );
        }
        println!("   Next: {}", finding.action);
    }
    if g.findings.len() > limit {
        println!("\nShowing {limit} of {} findings. Use --details for all findings or --json for structured output.", g.findings.len());
    }
}
