use serde::Deserialize;
use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
};

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TsConfig {
    extends: Option<String>,
    #[serde(default)]
    compiler_options: CompilerOptions,
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

impl ResolverConfig {
    pub fn load(root: &Path) -> Self {
        load_file(&root.join("tsconfig.json"), 0)
    }
}
fn load_file(path: &Path, depth: u8) -> ResolverConfig {
    if depth > 8 {
        return ResolverConfig::default();
    }
    let Ok(raw) = fs::read_to_string(path) else {
        return ResolverConfig::default();
    };
    let clean = clean_jsonc(&raw);
    let Ok(config) = serde_json::from_str::<TsConfig>(&clean) else {
        return ResolverConfig::default();
    };
    let mut merged = config
        .extends
        .as_deref()
        .and_then(|ext| resolve_extends(path, ext))
        .map(|p| load_file(&p, depth + 1))
        .unwrap_or_default();
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
        merged.paths = paths;
        merged.paths_base = path
            .parent()
            .map(|parent| parent.to_string_lossy().into_owned());
    }
    merged
}
fn resolve_extends(current: &Path, ext: &str) -> Option<PathBuf> {
    if !ext.starts_with('.') {
        return None;
    }
    let p = current.parent()?.join(ext);
    if p.is_file() {
        return Some(p);
    }
    let mut name = p.into_os_string();
    name.push(".json");
    Some(PathBuf::from(name))
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
