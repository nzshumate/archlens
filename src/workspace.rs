use anyhow::{bail, Context, Result};
use serde::Deserialize;
use serde_json::Value;
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};
#[derive(Deserialize)]
struct Package {
    name: Option<String>,
    source: Option<String>,
    module: Option<String>,
    main: Option<String>,
    exports: Option<Value>,
}
pub struct Workspaces {
    packages: BTreeMap<String, (PathBuf, Package)>,
}
impl Workspaces {
    pub fn discover(files: &[PathBuf]) -> Result<Self> {
        let mut packages = BTreeMap::new();
        for path in files
            .iter()
            .filter(|p| p.file_name().is_some_and(|n| n == "package.json"))
        {
            let raw = fs::read_to_string(path)?;
            let package: Package = serde_json::from_str(&raw)
                .with_context(|| format!("invalid package manifest {}", path.display()))?;
            if let Some(name) = &package.name {
                if packages.contains_key(name) {
                    bail!(
                        "duplicate internal package name '{name}' in {}",
                        path.display()
                    );
                }
                packages.insert(
                    name.clone(),
                    (path.parent().unwrap().to_path_buf(), package),
                );
            }
        }
        Ok(Self { packages })
    }
    pub fn contains(&self, spec: &str) -> bool {
        self.package(spec).is_some()
    }
    fn package(&self, spec: &str) -> Option<(&str, &Path, &Package)> {
        self.packages
            .iter()
            .filter(|(name, _)| spec == name.as_str() || spec.starts_with(&format!("{name}/")))
            .max_by_key(|(name, _)| name.len())
            .map(|(name, (dir, package))| (name.as_str(), dir.as_path(), package))
    }
    pub fn candidates(&self, spec: &str) -> Vec<PathBuf> {
        let Some((name, directory, package)) = self.package(spec) else {
            return Vec::new();
        };
        let suffix = spec.strip_prefix(name).unwrap().trim_start_matches('/');
        if suffix.split('/').any(|part| part == "..") {
            return Vec::new();
        }
        // Source is an explicit source-graph preference over generated package outputs.
        if suffix.is_empty() {
            if let Some(source) = &package.source {
                return vec![directory.join(source)];
            }
        }
        if let Some(exports) = &package.exports {
            let key = if suffix.is_empty() {
                ".".to_owned()
            } else {
                format!("./{suffix}")
            };
            let mut wildcard = None;
            let target = if let Some(map) = exports
                .as_object()
                .filter(|map| map.keys().any(|key| key.starts_with('.')))
            {
                if let Some(exact) = map.get(&key) {
                    Some(exact)
                } else {
                    let mut patterns = map
                        .iter()
                        .filter_map(|(pattern, target)| {
                            let (prefix, ending) = pattern.split_once('*')?;
                            let matched = key.strip_prefix(prefix)?.strip_suffix(ending)?;
                            Some((prefix.len(), ending.len(), matched, target))
                        })
                        .collect::<Vec<_>>();
                    patterns.sort_by_key(|(prefix, ending, _, _)| {
                        std::cmp::Reverse((*prefix, *ending))
                    });
                    patterns.first().map(|(_, _, matched, target)| {
                        wildcard = Some(*matched);
                        *target
                    })
                }
            } else if suffix.is_empty() {
                Some(exports)
            } else {
                None
            };
            let Some(target) = target else {
                return Vec::new();
            };
            return export_targets(target)
                .into_iter()
                .filter_map(|target| {
                    let target = wildcard
                        .map_or_else(|| target.to_owned(), |value| target.replace('*', value));
                    if !target.starts_with("./") || target.split('/').any(|p| p == "..") {
                        return None;
                    }
                    Some(directory.join(target))
                })
                .collect();
        }
        if !suffix.is_empty() {
            return vec![directory.join(suffix), directory.join("src").join(suffix)];
        }
        package
            .module
            .iter()
            .chain(package.main.iter())
            .map(|target| directory.join(target))
            .chain([directory.join("src/index"), directory.join("index")])
            .collect()
    }
}
// Static frontend-analysis conditions. This deliberately does not emulate every
// runtime's export conditions; unsupported conditions stay unresolved and visible.
fn export_targets(value: &Value) -> Vec<&str> {
    match value {
        Value::String(target) => vec![target],
        Value::Array(targets) => targets.iter().flat_map(export_targets).collect(),
        Value::Object(conditions) => {
            for condition in ["source", "browser", "import", "default", "require", "types"] {
                if let Some(value) = conditions.get(condition) {
                    return export_targets(value);
                }
            }
            Vec::new()
        }
        _ => Vec::new(),
    }
}
