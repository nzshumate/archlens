use serde::Deserialize;
use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
};
use walkdir::WalkDir;
#[derive(Deserialize)]
struct Package {
    name: Option<String>,
}
#[derive(Default)]
pub struct Workspaces {
    packages: HashMap<String, PathBuf>,
}
impl Workspaces {
    pub fn discover(root: &Path) -> Self {
        let mut packages = HashMap::new();
        for entry in WalkDir::new(root)
            .max_depth(4)
            .into_iter()
            .filter_entry(|e| {
                e.file_name() != "node_modules"
                    && e.file_name() != "target"
                    && e.file_name() != ".git"
            })
            .flatten()
        {
            if entry.file_type().is_file() && entry.file_name() == "package.json" {
                if let Ok(raw) = fs::read_to_string(entry.path()) {
                    if let Ok(pkg) = serde_json::from_str::<Package>(&raw) {
                        if let (Some(name), Some(dir)) = (pkg.name, entry.path().parent()) {
                            packages.insert(name, dir.to_path_buf());
                        }
                    }
                }
            }
        }
        Self { packages }
    }
    pub fn resolve(&self, spec: &str) -> Option<PathBuf> {
        let mut best: Option<(&str, &PathBuf)> = None;
        for (name, dir) in &self.packages {
            if (spec == name || spec.starts_with(&format!("{name}/")))
                && best.as_ref().is_none_or(|(old, _)| name.len() > old.len())
            {
                best = Some((name, dir));
            }
        }
        let (name, dir) = best?;
        let suffix = spec.strip_prefix(name)?.trim_start_matches('/');
        Some(if suffix.is_empty() {
            dir.join("src/index")
        } else {
            dir.join("src").join(suffix)
        })
    }
}
