use anyhow::Result;
use clap::{Parser, Subcommand};
use std::path::PathBuf;

mod analyzer;
mod config;
mod metrics;
mod parser;

#[derive(Parser)]
#[command(name = "archlens", version, about = "See your frontend architecture.")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Analyze a TypeScript/JavaScript frontend project.
    Analyze {
        #[arg(default_value = ".")]
        path: PathBuf,
        /// Emit the complete report as machine-readable JSON.
        #[arg(long)]
        json: bool,
    },
    /// Fail when architecture health falls below a threshold or cycles exist.
    Check {
        #[arg(default_value = ".")]
        path: PathBuf,
        #[arg(long, default_value_t = 70)]
        min_health: u8,
        #[arg(long)]
        allow_cycles: bool,
    },
}

fn main() -> Result<()> {
    match Cli::parse().command {
        Commands::Analyze { path, json } => {
            let report = analyzer::analyze(&path)?;
            let metrics = metrics::calculate(&report);
            if json {
                println!("{}", serde_json::json!({ "analysis": report, "metrics": metrics }));
            } else {
                println!("Archlens — {}", path.display());
                println!("Files scanned:          {}", report.files_scanned);
                println!("Source files:           {}", report.source_files);
                println!("Dependencies:           {}", report.dependencies);
                println!("Circular dependencies:  {}", report.cycles.len());
                println!("Orphan modules:         {}", metrics.orphan_modules.len());
                println!("Architecture health:    {}/100", metrics.health_score);
                for cycle in &report.cycles {
                    println!("  ⚠ {}", cycle.join(" -> "));
                }
            }
        }
        Commands::Check { path, min_health, allow_cycles } => {
            let report = analyzer::analyze(&path)?;
            let metrics = metrics::calculate(&report);
            let cycles_fail = !allow_cycles && !report.cycles.is_empty();
            let health_fail = metrics.health_score < min_health;
            println!("Architecture health: {}/100", metrics.health_score);
            println!("Cycles: {}", report.cycles.len());
            if cycles_fail || health_fail {
                std::process::exit(1);
            }
        }
    }
    Ok(())
}
