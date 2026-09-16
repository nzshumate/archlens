use anyhow::Result;
use clap::{Parser, Subcommand};
use std::path::PathBuf;
mod analyzer; mod config; mod git; mod metrics; mod parser; mod rules; mod server;

#[derive(Parser)] #[command(name="archlens", version, about="See your frontend architecture.")] struct Cli { #[command(subcommand)] command: Commands }
#[derive(Subcommand)] enum Commands {
    /// Analyze a TypeScript/JavaScript frontend project.
    Analyze { #[arg(default_value=".")] path: PathBuf, #[arg(long)] json: bool },
    /// Enforce health, cycles, and archlens.json boundary rules for CI.
    Check { #[arg(default_value=".")] path: PathBuf, #[arg(long, default_value_t=70)] min_health: u8, #[arg(long)] allow_cycles: bool },
    /// Report architectural impact of changes since a Git base ref.
    Diff { base: String, #[arg(default_value=".")] path: PathBuf, #[arg(long)] json: bool },
    /// Launch the local architecture explorer.
    Dev { #[arg(default_value=".")] path: PathBuf, #[arg(long, default_value_t=4242)] port: u16 },
}

fn main() -> Result<()> {
    match Cli::parse().command {
        Commands::Analyze { path, json } => {
            let report=analyzer::analyze(&path)?; let metrics=metrics::calculate(&report); let violations=rules::evaluate(&report,&rules::load(&path));
            if json { println!("{}", serde_json::to_string_pretty(&serde_json::json!({"analysis":report,"metrics":metrics,"violations":violations}))?); }
            else { println!("Archlens — {}",path.display()); println!("Files scanned:          {}",report.files_scanned); println!("Source files:           {}",report.source_files); println!("Dependencies:           {}",report.dependencies); println!("Circular dependencies:  {}",report.cycles.len()); println!("Dead candidates:        {}",metrics.dead_candidates.len()); println!("Large modules:          {}",metrics.large_modules.len()); println!("Boundary violations:    {}",violations.len()); println!("Architecture health:    {}/100",metrics.health_score); for cycle in &report.cycles { println!("  ⚠ {}",cycle.join(" -> ")); } }
        }
        Commands::Check { path,min_health,allow_cycles } => {
            let report=analyzer::analyze(&path)?; let metrics=metrics::calculate(&report); let violations=rules::evaluate(&report,&rules::load(&path));
            println!("Architecture health: {}/100",metrics.health_score); println!("Cycles: {}",report.cycles.len()); println!("Boundary violations: {}",violations.len());
            for violation in &violations { println!("  {} -> {}: {}",violation.from,violation.to,violation.message); }
            if (!allow_cycles && !report.cycles.is_empty()) || metrics.health_score < min_health || !violations.is_empty() { std::process::exit(1); }
        }
        Commands::Diff { base,path,json } => { let report=analyzer::analyze(&path)?; let diff=git::diff(&path,&base,&report)?; if json { println!("{}",serde_json::to_string_pretty(&diff)?); } else { println!("Changed source files: {}",diff.changed_source_files.len()); println!("Affected dependencies: {}",diff.affected_dependencies); println!("Cycles in current tree: {}",diff.cycles_in_current_tree); for file in diff.changed_source_files { println!("  {file}"); } } }
        Commands::Dev { path,port } => server::serve(&path,port)?,
    } Ok(())
}
