use anyhow::Result;
use clap::{Parser, Subcommand};
use std::path::PathBuf;

mod analyzer;

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
        /// Project directory to scan.
        #[arg(default_value = ".")]
        path: PathBuf,
        /// Emit machine-readable JSON.
        #[arg(long)]
        json: bool,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Analyze { path, json } => {
            let report = analyzer::analyze(&path)?;
            if json {
                println!("{}", serde_json::to_string_pretty(&report)?);
            } else {
                println!("Archlens — {}", path.display());
                println!("Files scanned:          {}", report.files_scanned);
                println!("Source files:           {}", report.source_files);
                println!("Dependencies:           {}", report.dependencies);
                println!("Circular dependencies:  {}", report.cycles.len());
                for cycle in &report.cycles {
                    println!("  ⚠ {}", cycle.join(" -> "));
                }
            }
        }
    }

    Ok(())
}
