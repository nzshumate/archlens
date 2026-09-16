use crate::analyzer::AnalysisReport;
use serde::{Deserialize,Serialize};
use std::{fs,path::Path,process::Command};

#[derive(Serialize,Deserialize)] struct Cached { head:String, report:AnalysisReport }
fn git(root:&Path,args:&[&str])->Option<String>{let out=Command::new("git").args(args).current_dir(root).output().ok()?;out.status.success().then(||String::from_utf8_lossy(&out.stdout).trim().to_string())}
fn clean_head(root:&Path)->Option<String>{let status=git(root,&["status","--porcelain"])?;if !status.is_empty(){return None;}git(root,&["rev-parse","HEAD"])}
pub fn load(root:&Path)->Option<AnalysisReport>{let head=clean_head(root)?;let raw=fs::read_to_string(root.join("target/archlens/report.json")).ok()?;let cached:Cached=serde_json::from_str(&raw).ok()?;(cached.head==head).then_some(cached.report)}
pub fn save(root:&Path,report:&AnalysisReport){let Some(head)=clean_head(root)else{return;};let dir=root.join("target/archlens");if fs::create_dir_all(&dir).is_err(){return;}if let Ok(raw)=serde_json::to_string(&Cached{head,report:report.clone()}){let _=fs::write(dir.join("report.json"),raw);}}
