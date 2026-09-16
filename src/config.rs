use regex::Regex;
use serde::Deserialize;
use std::{collections::HashMap, fs, path::{Path,PathBuf}};

#[derive(Debug,Default,Deserialize)] #[serde(rename_all="camelCase")] struct TsConfig { extends:Option<String>, #[serde(default)] compiler_options:CompilerOptions }
#[derive(Debug,Default,Deserialize)] #[serde(rename_all="camelCase")] struct CompilerOptions { base_url:Option<String>, #[serde(default)] paths:HashMap<String,Vec<String>> }
#[derive(Debug,Default)] pub struct ResolverConfig { pub base_url:Option<String>, pub paths:HashMap<String,Vec<String>> }

impl ResolverConfig { pub fn load(root:&Path)->Self { load_file(&root.join("tsconfig.json"),0) } }
fn load_file(path:&Path,depth:u8)->ResolverConfig {
    if depth>8{return ResolverConfig::default();} let Ok(raw)=fs::read_to_string(path) else{return ResolverConfig::default();}; let clean=clean_jsonc(&raw); let Ok(config)=serde_json::from_str::<TsConfig>(&clean) else{return ResolverConfig::default();};
    let mut merged=config.extends.as_deref().and_then(|ext|resolve_extends(path,ext)).map(|p|load_file(&p,depth+1)).unwrap_or_default();
    if config.compiler_options.base_url.is_some(){merged.base_url=config.compiler_options.base_url;} for(k,v)in config.compiler_options.paths{merged.paths.insert(k,v);} merged
}
fn resolve_extends(current:&Path,ext:&str)->Option<PathBuf>{if !ext.starts_with('.') {return None;} let mut p=current.parent()?.join(ext); if p.extension().is_none(){p.set_extension("json");} Some(p)}
fn clean_jsonc(raw:&str)->String { let block=Regex::new(r"(?s)/\*.*?\*/").expect("valid regex"); let line=Regex::new(r"(?m)//.*$").expect("valid regex"); let trailing=Regex::new(r",\s*([}\]])").expect("valid regex"); let a=block.replace_all(raw,""); let b=line.replace_all(&a,""); trailing.replace_all(&b,"$1").into_owned() }

#[cfg(test)] mod tests {use super::*; #[test] fn strips_jsonc(){let raw="{ // x\n \"compilerOptions\": {\"paths\": {},}, /* y */ }"; assert!(serde_json::from_str::<serde_json::Value>(&clean_jsonc(raw)).is_ok());}}
