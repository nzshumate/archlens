use anyhow::Result;
use clap::{Parser, Subcommand};
use std::path::PathBuf;
mod analyzer;
mod cache;
mod config;
mod discovery;
mod framework;
mod git;
mod metrics;
mod parser;
mod rules;
mod server;
mod workspace;
#[derive(Parser)]
#[command(name = "oxarch", version, about = "See your frontend architecture.")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}
#[derive(Subcommand)]
enum Commands {
    /// Analyze dependencies, diagnostics, and architecture health.
    Analyze {
        #[arg(default_value = ".")]
        path: PathBuf,
        #[arg(long)]
        json: bool,
        #[arg(long)]
        no_cache: bool,
    },
    /// Enforce boundaries and health in CI.
    Check {
        #[arg(default_value = ".")]
        path: PathBuf,
        #[arg(long, default_value_t = 70, value_parser = clap::value_parser!(u8).range(0..=100))]
        min_health: u8,
        #[arg(long)]
        allow_cycles: bool,
        /// Emit JSON on pass/check failure; fatal analysis errors go to stderr.
        #[arg(long)]
        json: bool,
        /// Also fail on unresolved internal imports.
        #[arg(long)]
        strict: bool,
    },
    /// Compare the working tree with a Git merge base.
    Diff {
        base: String,
        #[arg(default_value = ".")]
        path: PathBuf,
        #[arg(long)]
        json: bool,
    },
    /// Open a local interactive architecture explorer.
    Dev {
        #[arg(default_value = ".")]
        path: PathBuf,
        #[arg(long, default_value_t = 4242)]
        port: u16,
        #[arg(long)]
        base: Option<String>,
    },
}
fn main() -> Result<()> {
    match Cli::parse().command {
        Commands::Analyze {
            path,
            json,
            no_cache,
        } => {
            let report = analyzer::analyze_with_cache(&path, !no_cache)?;
            let config = rules::load(&path)?;
            let metrics = metrics::calculate(&report, &config)?;
            let violations = rules::evaluate(&report, &config);
            if json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(
                        &serde_json::json!({"schema_version":1,"analysis":report,"metrics":metrics,"violations":violations})
                    )?
                );
            } else {
                println!("Oxarch — {}", path.display());
                println!("Files scanned:          {}", report.files_scanned);
                println!("Source files:           {}", report.source_files);
                println!("Dependencies:           {}", report.dependencies);
                println!("Circular dependencies:  {}", report.cycles.len());
                println!("Dead candidates:        {}", metrics.dead_candidates.len());
                println!("Large modules:          {}", metrics.large_modules.len());
                println!("Boundary violations:    {}", violations.len());
                println!("Architecture health:    {}/100", metrics.health_score);
                println!("Reachability mode:      {}", metrics.reachability_mode);
                println!("Entry points:           {}", metrics.entry_points.len());
                print_diagnostics(&report);
                for cycle in &report.cycles {
                    println!("  ⚠ {}", cycle.join(" -> "));
                }
            }
        }
        Commands::Check {
            path,
            min_health,
            allow_cycles,
            json,
            strict,
        } => {
            let report = analyzer::analyze(&path)?;
            let config = rules::load(&path)?;
            let metrics = metrics::calculate(&report, &config)?;
            let violations = rules::evaluate(&report, &config);
            let mut failures = Vec::new();
            if !allow_cycles && !report.cycles.is_empty() {
                failures.push("cycles");
            }
            if metrics.health_score < min_health {
                failures.push("health");
            }
            if !violations.is_empty() {
                failures.push("boundaries");
            }
            if report.diagnostics.iter().any(|d| d.code == "parse_error") {
                failures.push("parse_errors");
            }
            if report
                .diagnostics
                .iter()
                .any(|d| d.code == "unresolved_config")
            {
                failures.push("configuration");
            }
            if strict
                && report
                    .diagnostics
                    .iter()
                    .any(|d| d.code == "unresolved_import")
            {
                failures.push("unresolved_imports");
            }
            if report.source_files == 0 {
                failures.push("no_source_files");
            }
            if json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&serde_json::json!({
                        "schema_version": 1, "analysis": report, "metrics": metrics, "violations": violations,
                        "check": {"passed": failures.is_empty(), "failures": failures, "min_health": min_health,
                                  "allow_cycles": allow_cycles, "strict": strict}
                    }))?
                );
            } else {
                println!("Architecture health: {}/100", metrics.health_score);
                println!("Cycles: {}", report.cycles.len());
                println!("Boundary violations: {}", violations.len());
                for v in &violations {
                    println!("  {} -> {}: {}", v.from, v.to, v.message);
                }
                print_diagnostics(&report);
                if !failures.is_empty() {
                    println!("Failed checks: {}", failures.join(", "));
                }
            }
            if !failures.is_empty() {
                std::process::exit(1);
            }
        }

        Commands::Diff { base, path, json } => {
            let report = analyzer::analyze(&path)?;
            let diff = git::diff(&path, &base, &report)?;
            if json {
                println!("{}", serde_json::to_string_pretty(&diff)?);
            } else {
                println!("Merge base: {}", diff.merge_base);
                if !diff.analysis_complete {
                    println!("Warning: comparison contains analysis diagnostics; review JSON output for details.");
                }
                println!("Changed source files: {}", diff.changed_source_files.len());
                println!("Deleted source files: {}", diff.deleted_source_files.len());
                println!("Affected modules: {}", diff.affected_modules.len());
                println!("Affected dependencies: {}", diff.affected_dependencies);
                println!("Added dependencies: {}", diff.added_dependencies.len());
                println!("Removed dependencies: {}", diff.removed_dependencies.len());
                println!("New cycles: {}", diff.new_cycles.len());
                for file in diff.changed_source_files {
                    println!("  {file}");
                }
            }
        }
        Commands::Dev { path, port, base } => server::serve(&path, port, base.as_deref())?,
    }
    Ok(())
}

fn print_diagnostics(report: &analyzer::AnalysisReport) {
    println!("Analysis diagnostics:   {}", report.diagnostics.len());
    if !report.diagnostics.is_empty() {
        println!("Graph and health score may be incomplete; review diagnostics below.");
    }
    for diagnostic in &report.diagnostics {
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
        println!(
            "  [{code}] {location}: {message}",
            code = diagnostic.code,
            message = diagnostic.message
        );
    }
}
