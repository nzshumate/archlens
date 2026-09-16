use std::{fs,process::Command};
use tempfile::tempdir;

#[test]
fn analyze_json_reports_dependency_graph() {
    let dir=tempdir().unwrap(); fs::write(dir.path().join("a.ts"),"import './b';").unwrap(); fs::write(dir.path().join("b.ts"),"export const b=1;").unwrap();
    let output=Command::new(env!("CARGO_BIN_EXE_archlens")).args(["analyze",dir.path().to_str().unwrap(),"--json"]).output().unwrap();
    assert!(output.status.success()); let json:serde_json::Value=serde_json::from_slice(&output.stdout).unwrap(); assert_eq!(json["analysis"]["dependencies"],1); assert_eq!(json["analysis"]["source_files"],2);
}

#[test]
fn check_fails_on_cycle() {
    let dir=tempdir().unwrap(); fs::write(dir.path().join("a.ts"),"import './b';").unwrap(); fs::write(dir.path().join("b.ts"),"import './a';").unwrap();
    let status=Command::new(env!("CARGO_BIN_EXE_archlens")).args(["check",dir.path().to_str().unwrap()]).status().unwrap(); assert!(!status.success());
}
