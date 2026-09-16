use serde::Deserialize;
use std::{collections::HashMap, fs, path::Path};

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TsConfig {
    #[serde(default)]
    compiler_options: CompilerOptions,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CompilerOptions {
    base_url: Option<String>,
    #[serde(default)]
    paths: HashMap<String, Vec<String>>,
}

#[derive(Debug, Default)]
pub struct ResolverConfig {
    pub base_url: Option<String>,
    pub paths: HashMap<String, Vec<String>>,
}

impl ResolverConfig {
    pub fn load(root: &Path) -> Self {
        let path = root.join("tsconfig.json");
        let Ok(raw) = fs::read_to_string(path) else { return Self::default(); };
        // Strip the most common JSONC form so ordinary tsconfig files work without
        // pulling a full JSONC parser into the first release.
        let comments = regex::Regex::new(r"(?m)//.*$").unwrap();
        let raw = comments.replace_all(&raw, "");
        let Ok(config) = serde_json::from_str::<TsConfig>(&raw) else { return Self::default(); };
        Self { base_url: config.compiler_options.base_url, paths: config.compiler_options.paths }
    }
}
