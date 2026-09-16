use anyhow::{Context, Result};
use ignore::WalkBuilder;
use std::path::{Path, PathBuf};

/// Project-local ignore rules keep results independent of the user's global Git config.
pub fn files(root: &Path) -> Result<Vec<PathBuf>> {
    anyhow::ensure!(
        root.is_dir(),
        "analysis path must be a directory: {}",
        root.display()
    );
    let mut builder = WalkBuilder::new(root);
    builder
        .hidden(false)
        .parents(false)
        .git_global(false)
        .git_exclude(false)
        .ignore(false)
        .require_git(false)
        .add_custom_ignore_filename(".oxarchignore")
        .filter_entry(|entry| {
            entry.depth() == 0
                || !matches!(
                    entry.file_name().to_str(),
                    Some(
                        "node_modules"
                            | ".git"
                            | "dist"
                            | "build"
                            | ".next"
                            | ".nuxt"
                            | "coverage"
                            | "target"
                    )
                )
        });
    let mut files = Vec::new();
    for entry in builder.build() {
        let entry = entry.context("cannot discover project files")?;
        if let Some(error) = entry.error() {
            anyhow::bail!("invalid ignore configuration: {error}");
        }
        if entry.file_type().is_some_and(|kind| kind.is_file()) {
            files.push(entry.into_path());
        }
    }
    files.sort();
    Ok(files)
}
