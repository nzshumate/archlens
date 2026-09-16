use anyhow::{bail, Context, Result};
use serde::Deserialize;
use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
};

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TsConfig {
    extends: Option<Extends>,
    #[serde(default)]
    compiler_options: CompilerOptions,
}
#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum Extends {
    One(String),
    Many(Vec<String>),
}
impl Extends {
    fn values(self) -> Vec<String> {
        match self {
            Self::One(value) => vec![value],
            Self::Many(values) => values,
        }
    }
}
pub struct ConfigIssue {
    pub file: PathBuf,
    pub message: String,
}
#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CompilerOptions {
    base_url: Option<String>,
    #[serde(default)]
    paths: Option<HashMap<String, Vec<String>>>,
}
#[derive(Debug, Default)]
pub struct ResolverConfig {
    pub base_url: Option<String>,
    pub paths: HashMap<String, Vec<String>>,
    pub paths_base: Option<String>,
}

pub struct Configs {
    root: PathBuf,
    by_directory: HashMap<PathBuf, ResolverConfig>,
    fallback: ResolverConfig,
    pub issues: Vec<ConfigIssue>,
}
impl Configs {
    pub fn load(root: &Path, files: &[PathBuf]) -> Result<Self> {
        let mut by_directory = HashMap::new();
        let mut issues = Vec::new();
        for path in files.iter().filter(|path| {
            matches!(
                path.file_name().and_then(|n| n.to_str()),
                Some("tsconfig.json" | "jsconfig.json")
            )
        }) {
            let directory = path.parent().expect("discovered files have parents");
            if path.file_name().is_some_and(|name| name == "jsconfig.json")
                && files
                    .binary_search(&directory.join("tsconfig.json"))
                    .is_ok()
            {
                continue;
            }
            by_directory.insert(
                directory.to_path_buf(),
                load_file(path, &mut Vec::new(), &mut issues)?,
            );
        }
        Ok(Self {
            root: root.to_path_buf(),
            by_directory,
            fallback: ResolverConfig::default(),
            issues,
        })
    }
    pub fn for_source(&self, source: &Path) -> &ResolverConfig {
        for directory in source.parent().into_iter().flat_map(Path::ancestors) {
            if let Some(config) = self.by_directory.get(directory) {
                return config;
            }
            if directory == self.root {
                break;
            }
        }
        &self.fallback
    }
}
fn load_file(
    path: &Path,
    stack: &mut Vec<PathBuf>,
    issues: &mut Vec<ConfigIssue>,
) -> Result<ResolverConfig> {
    let path = path
        .canonicalize()
        .with_context(|| format!("cannot open TypeScript configuration {}", path.display()))?;
    if stack.contains(&path) {
        bail!(
            "circular TypeScript configuration extends: {}",
            path.display()
        );
    }
    if stack.len() >= 32 {
        bail!(
            "TypeScript configuration extends exceeds 32 levels: {}",
            path.display()
        );
    }
    stack.push(path.clone());
    let raw =
        fs::read_to_string(&path).with_context(|| format!("cannot read {}", path.display()))?;
    let config: TsConfig = serde_json::from_str(&clean_jsonc(&raw))
        .with_context(|| format!("invalid TypeScript configuration {}", path.display()))?;
    let mut merged = ResolverConfig::default();
    for ext in config.extends.map(Extends::values).unwrap_or_default() {
        if let Some(parent) = resolve_extends(&path, &ext)? {
            let inherited = load_file(&parent, stack, issues)?;
            if inherited.base_url.is_some() {
                merged.base_url = inherited.base_url;
            }
            if inherited.paths_base.is_some() {
                merged.paths = inherited.paths;
                merged.paths_base = inherited.paths_base;
            }
        } else {
            issues.push(ConfigIssue {
                file: path.clone(),
                message: format!("Cannot resolve shared configuration '{ext}' from local node_modules. Install this project's dependencies or correct extends; inherited aliases may be missing."),
            });
        }
    }
    if let Some(base_url) = config.compiler_options.base_url {
        merged.base_url = Some(
            path.parent()
                .unwrap_or(Path::new("."))
                .join(base_url)
                .to_string_lossy()
                .into_owned(),
        );
    }
    if let Some(paths) = config.compiler_options.paths {
        for (alias, targets) in &paths {
            if alias.matches('*').count() > 1
                || targets.iter().any(|target| target.matches('*').count() > 1)
            {
                bail!(
                    "TypeScript paths support at most one wildcard: {alias} in {}",
                    path.display()
                );
            }
        }
        merged.paths = paths;
        merged.paths_base = path
            .parent()
            .map(|parent| parent.to_string_lossy().into_owned());
    }
    stack.pop();
    Ok(merged)
}
fn json_candidate(path: &Path) -> Option<PathBuf> {
    if path.is_file() {
        return Some(path.to_path_buf());
    }
    let mut name = path.as_os_str().to_os_string();
    name.push(".json");
    let candidate = PathBuf::from(name);
    candidate.is_file().then_some(candidate)
}
fn resolve_extends(current: &Path, ext: &str) -> Result<Option<PathBuf>> {
    anyhow::ensure!(
        !ext.is_empty(),
        "empty TypeScript extends in {}",
        current.display()
    );
    if ext.starts_with('.') || Path::new(ext).is_absolute() {
        let path = current.parent().unwrap().join(ext);
        // Keep missing local files fatal, with their exact path in the error.
        return Ok(Some(json_candidate(&path).unwrap_or(path)));
    }
    anyhow::ensure!(
        !ext.contains('\\') && !ext.split('/').any(|part| part == ".." || part.is_empty()),
        "invalid package configuration reference '{ext}' in {}",
        current.display()
    );
    let parts = ext.split('/').collect::<Vec<_>>();
    let package_parts = if ext.starts_with('@') { 2 } else { 1 };
    anyhow::ensure!(
        parts.len() >= package_parts,
        "invalid scoped configuration reference '{ext}'"
    );
    let package = parts[..package_parts].join("/");
    let subpath = parts[package_parts..].join("/");
    for ancestor in current.parent().unwrap().ancestors() {
        let directory = ancestor.join("node_modules").join(&package);
        if !directory.is_dir() {
            continue;
        }
        if !subpath.is_empty() {
            if let Some(path) = json_candidate(&directory.join(&subpath)) {
                return Ok(Some(path));
            }
        } else {
            let manifest = directory.join("package.json");
            if manifest.is_file() {
                let value: serde_json::Value =
                    serde_json::from_str(&fs::read_to_string(&manifest)?).with_context(|| {
                        format!(
                            "invalid configuration package manifest {}",
                            manifest.display()
                        )
                    })?;
                if let Some(target) = value.get("tsconfig").and_then(serde_json::Value::as_str) {
                    if let Some(path) = json_candidate(&directory.join(target)) {
                        return Ok(Some(path));
                    }
                }
            }
            if let Some(path) = json_candidate(&directory.join("tsconfig.json")) {
                return Ok(Some(path));
            }
        }
    }
    Ok(None)
}
fn clean_jsonc(raw: &str) -> String {
    let mut bytes = raw.as_bytes().to_vec();
    let mut i = 0;
    let mut quoted = false;
    while i < bytes.len() {
        if quoted {
            if bytes[i] == b'\\' {
                i += 2;
                continue;
            }
            if bytes[i] == b'"' {
                quoted = false;
            }
        } else if bytes[i] == b'"' {
            quoted = true;
        } else if bytes[i..].starts_with(b"//") {
            while i < bytes.len() && bytes[i] != b'\n' {
                bytes[i] = b' ';
                i += 1;
            }
            continue;
        } else if bytes[i..].starts_with(b"/*") {
            let comment_start = i;
            bytes[i] = b' ';
            bytes[i + 1] = b' ';
            i += 2;
            while i < bytes.len() && !bytes[i..].starts_with(b"*/") {
                bytes[i] = b' ';
                i += 1;
            }
            if i + 1 < bytes.len() {
                bytes[i] = b' ';
                bytes[i + 1] = b' ';
                i += 2;
            } else {
                bytes[comment_start] = b'/';
                bytes[comment_start + 1] = b'*';
            }
            continue;
        }
        i += 1;
    }
    i = 0;
    quoted = false;
    while i < bytes.len() {
        if quoted {
            if bytes[i] == b'\\' {
                i += 2;
                continue;
            }
            if bytes[i] == b'"' {
                quoted = false;
            }
        } else if bytes[i] == b'"' {
            quoted = true;
        } else if bytes[i] == b',' {
            let next = bytes[i + 1..]
                .iter()
                .find(|byte| !byte.is_ascii_whitespace());
            if matches!(next, Some(b'}' | b']')) {
                bytes[i] = b' ';
            }
        }
        i += 1;
    }
    String::from_utf8(bytes).expect("only ASCII syntax bytes are replaced")
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn preserves_jsonc_string_contents() {
        let raw = r#"{"url":"https://example.com", "alias":"@/*", "text":",}",}"#;
        let json: serde_json::Value = serde_json::from_str(&clean_jsonc(raw)).unwrap();
        assert_eq!(json["url"], "https://example.com");
        assert_eq!(json["alias"], "@/*");
        assert_eq!(json["text"], ",}");
    }
    #[test]
    fn strips_jsonc() {
        let raw = "{ // x\n \"compilerOptions\": {\"paths\": {},}, /* y */ }";
        assert!(serde_json::from_str::<serde_json::Value>(&clean_jsonc(raw)).is_ok());
    }
}
