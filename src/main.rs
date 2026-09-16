use anyhow::Result;
use clap::{Parser, Subcommand};
use std::path::PathBuf;
mod advice;
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
        /// Show all findings, file paths, and supporting evidence.
        #[arg(long)]
        details: bool,
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
        /// Show all findings, file paths, and supporting evidence.
        #[arg(long)]
        details: bool,
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
            details,
        } => {
            let report = analyzer::analyze_with_cache(&path, !no_cache)?;
            let config = rules::load(&path)?;
            let metrics = metrics::calculate(&report, &config)?;
            let violations = rules::evaluate(&report, &config);
            let guidance = advice::build(&report, &metrics, &config, &violations);
            if json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(
                        &serde_json::json!({"schema_version":1,"analysis":report,"metrics":metrics,"violations":violations,"guidance":guidance})
                    )?
                );
            } else {
                println!("Oxarch — {}", path.display());
                println!(
                    "{} source modules | {} internal dependencies | {} cycle groups",
                    report.source_files,
                    report.dependencies,
                    report.cycles.len()
                );
                advice::print(&guidance, details);
            }
        }
        Commands::Check {
            path,
            min_health,
            allow_cycles,
            json,
            strict,
            details,
        } => {
            let report = analyzer::analyze(&path)?;
            let config = rules::load(&path)?;
            let metrics = metrics::calculate(&report, &config)?;
            let violations = rules::evaluate(&report, &config);
            let guidance = advice::build(&report, &metrics, &config, &violations);
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
                        "schema_version": 1, "analysis": report, "metrics": metrics, "violations": violations, "guidance": guidance,
                        "check": {"passed": failures.is_empty(), "failures": failures, "min_health": min_health,
                                  "allow_cycles": allow_cycles, "strict": strict}
                    }))?
                );
            } else {
                println!(
                    "Check {} (minimum score: {min_health}; cycles {}; unresolved imports {})",
                    if failures.is_empty() {
                        "PASSED"
                    } else {
                        "FAILED"
                    },
                    if allow_cycles { "allowed" } else { "rejected" },
                    if strict { "rejected" } else { "reported only" }
                );
                if !failures.is_empty() {
                    println!("Failed checks: {}", failures.join(", "));
                }
                advice::print(&guidance, details);
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
                println!("\nNext steps:");
                for step in &diff.next_steps {
                    println!("  - {step}");
                }
                for cycle in &diff.new_cycles {
                    println!("Cycle group: {}", cycle.join(", "));
                }
                println!("\nChanged files:");
                for file in &diff.changed_source_files {
                    println!("  {file}");
                }
                println!("\nAffected modules (first 10; use --json for all):");
                for file in diff.affected_modules.iter().take(10) {
                    println!("  {file}");
                }
                println!("\nImport changes (first 10 of each; use --json for all):");
                for edge in diff.added_edges.iter().take(10) {
                    println!("  Added: {} imports {}", edge.from, edge.to);
                }
                for edge in diff.removed_edges.iter().take(10) {
                    println!("  Removed: {} imports {}", edge.from, edge.to);
                }
            }
        }
        Commands::Dev { path, port, base } => server::serve(&path, port, base.as_deref())?,
    }
    Ok(())
}
