use std::{fs, process::Command};
use tempfile::tempdir;

#[test]
fn dotted_imports_resolve_to_the_full_module_name() {
    let dir = tempdir().unwrap();
    fs::write(dir.path().join("main.ts"), "import './user.service';").unwrap();
    fs::write(dir.path().join("user.service.ts"), "export const user = 1;").unwrap();
    // A similarly named module must not steal the dependency.
    fs::write(dir.path().join("user.ts"), "export const other = 2;").unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_oxarch"))
        .args(["analyze", dir.path().to_str().unwrap(), "--json"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(
        json["analysis"]["edges"],
        serde_json::json!([
            {"from": "main.ts", "to": "user.service.ts"}
        ])
    );
}

#[test]
fn analyze_json_reports_dependency_graph() {
    let dir = tempdir().unwrap();
    fs::write(dir.path().join("a.ts"), "import './b';").unwrap();
    fs::write(dir.path().join("b.ts"), "export const b=1;").unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_oxarch"))
        .args(["analyze", dir.path().to_str().unwrap(), "--json"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(json["analysis"]["dependencies"], 1);
    assert_eq!(json["analysis"]["source_files"], 2);
}

#[test]
fn check_fails_on_cycle() {
    let dir = tempdir().unwrap();
    fs::write(dir.path().join("a.ts"), "import './b';").unwrap();
    fs::write(dir.path().join("b.ts"), "import './a';").unwrap();
    let status = Command::new(env!("CARGO_BIN_EXE_oxarch"))
        .args(["check", dir.path().to_str().unwrap()])
        .status()
        .unwrap();
    assert!(!status.success());
}

fn analyze(path: &std::path::Path, no_cache: bool) -> serde_json::Value {
    let mut command = Command::new(env!("CARGO_BIN_EXE_oxarch"));
    command.args(["analyze", path.to_str().unwrap(), "--json"]);
    if no_cache {
        command.arg("--no-cache");
    }
    let output = command.output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}
#[test]
fn incremental_cache_tracks_edits_deletes_and_config_changes() {
    let dir = tempdir().unwrap();
    let root = dir.path();
    fs::write(root.join("main.ts"), "import '@lib';").unwrap();
    fs::write(root.join("a.ts"), "export const a=1;").unwrap();
    fs::write(root.join("b.ts"), "export const b=2;").unwrap();
    let config = |target: &str| {
        serde_json::json!({"compilerOptions":{"paths":{"@lib":[target]}}}).to_string()
    };
    fs::write(root.join("tsconfig.json"), config("a.ts")).unwrap();
    let cold = analyze(root, false);
    assert_eq!(cold, analyze(root, false));
    assert_eq!(cold, analyze(root, true));
    assert!(root.join("target/oxarch/parsed-v1.json").is_file());
    fs::write(root.join("tsconfig.json"), config("b.ts")).unwrap();
    let configured = analyze(root, false);
    assert_eq!(configured["analysis"]["edges"][0]["to"], "b.ts");
    assert_eq!(configured, analyze(root, true));
    fs::write(root.join("main.ts"), "import './a';").unwrap();
    let edited = analyze(root, false);
    assert_eq!(edited["analysis"]["edges"][0]["to"], "a.ts");
    fs::remove_file(root.join("a.ts")).unwrap();
    assert_eq!(analyze(root, false)["analysis"]["dependencies"], 0);
    let cache: serde_json::Value =
        serde_json::from_slice(&fs::read(root.join("target/oxarch/parsed-v1.json")).unwrap())
            .unwrap();
    assert!(cache["entries"].get("a.ts").is_none());
    fs::write(root.join("target/oxarch/parsed-v1.json"), "broken").unwrap();
    assert_eq!(analyze(root, false), analyze(root, true));
}
#[test]
fn repeated_imports_are_one_edge_and_self_import_is_a_cycle() {
    let dir = tempdir().unwrap();
    fs::write(
        dir.path().join("a.ts"),
        "import './a'; import {a} from './a';",
    )
    .unwrap();
    let result = analyze(dir.path(), false);
    assert_eq!(result["analysis"]["dependencies"], 1);
    assert_eq!(result["analysis"]["cycles"], serde_json::json!([["a.ts"]]));
}
fn git(root: &std::path::Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .args(args)
        .current_dir(root)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap().trim().into()
}
fn init(root: &std::path::Path) {
    git(root, &["init", "-b", "main"]);
    git(root, &["config", "commit.gpgsign", "false"]);
    git(root, &["config", "user.email", "test@example.com"]);
    git(root, &["config", "user.name", "Oxarch Test"]);
}
fn commit(root: &std::path::Path) {
    git(root, &["add", "."]);
    git(root, &["commit", "-m", "fixture"]);
}
fn diff(root: &std::path::Path, base: &str) -> serde_json::Value {
    let output = Command::new(env!("CARGO_BIN_EXE_oxarch"))
        .args(["diff", base, root.to_str().unwrap(), "--json"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}
#[test]
fn branch_diff_uses_merge_base_and_includes_dirty_deleted_and_nested_files() {
    let dir = tempdir().unwrap();
    let root = dir.path();
    init(root);
    let frontend = root.join("frontend");
    fs::create_dir(&frontend).unwrap();
    fs::write(frontend.join("main.ts"), "import './a';").unwrap();
    fs::write(frontend.join("a.ts"), "import './old';").unwrap();
    fs::write(frontend.join("old.ts"), "export const old=1;").unwrap();
    fs::write(root.join(".gitignore"), "target/\n").unwrap();
    commit(root);
    let base = git(root, &["rev-parse", "HEAD"]);
    git(root, &["checkout", "-b", "feature"]);
    fs::write(frontend.join("a.ts"), "import './new';").unwrap();
    fs::remove_file(frontend.join("old.ts")).unwrap();
    fs::write(frontend.join("new.ts"), "import './a';").unwrap();
    commit(root);
    git(root, &["checkout", "main"]);
    fs::write(frontend.join("main-only.ts"), "export {};").unwrap();
    commit(root);
    git(root, &["checkout", "feature"]);
    fs::write(frontend.join("dirty.ts"), "export {};").unwrap();
    let before = git(root, &["worktree", "list", "--porcelain"]);
    let report = diff(&frontend, "main");
    assert_eq!(report["merge_base"], base);
    assert_eq!(
        report["deleted_source_files"],
        serde_json::json!(["old.ts"])
    );
    assert_eq!(
        report["changed_source_files"],
        serde_json::json!(["a.ts", "dirty.ts", "new.ts", "old.ts"])
    );
    assert!(report["affected_modules"]
        .as_array()
        .unwrap()
        .contains(&serde_json::json!("main.ts")));
    assert_eq!(
        report["new_cycles"],
        serde_json::json!([["a.ts", "new.ts"]])
    );
    assert_eq!(
        report["removed_dependencies"],
        serde_json::json!(["a.ts -> old.ts"])
    );
    assert_eq!(git(root, &["worktree", "list", "--porcelain"]), before);
}
#[test]
fn cache_is_optional_when_its_directory_is_unwritable() {
    let dir = tempdir().unwrap();
    fs::write(dir.path().join("main.ts"), "export {};").unwrap();
    fs::write(dir.path().join("target"), "not a directory").unwrap();
    assert_eq!(analyze(dir.path(), false)["analysis"]["source_files"], 1);
}

#[test]
fn check_rejects_malformed_rules_and_enforces_boundaries() {
    let dir = tempdir().unwrap();
    let root = dir.path();
    fs::write(root.join("main.ts"), "import './data';").unwrap();
    fs::write(root.join("data.ts"), "export {};").unwrap();
    fs::write(root.join("oxarch.json"), "invalid").unwrap();
    let check = || {
        Command::new(env!("CARGO_BIN_EXE_oxarch"))
            .args([
                "check",
                root.to_str().unwrap(),
                "--min-health",
                "0",
                "--allow-cycles",
            ])
            .output()
            .unwrap()
    };
    let malformed = check();
    assert!(!malformed.status.success());
    assert!(String::from_utf8_lossy(&malformed.stderr).contains("invalid architecture rules"));
    fs::write(
        root.join("oxarch.json"),
        r#"{"boundaries":[{"from":"main.ts","disallow":"data.ts"}]}"#,
    )
    .unwrap();
    assert!(!check().status.success());
    fs::write(root.join("oxarch.json"), "{}").unwrap();
    assert!(check().status.success());
}
#[test]
fn branch_diff_supports_new_frontend_directories_and_invalid_refs() {
    let dir = tempdir().unwrap();
    let root = dir.path();
    init(root);
    fs::write(root.join("README.md"), "fixture").unwrap();
    commit(root);
    let frontend = root.join("new-frontend");
    fs::create_dir(&frontend).unwrap();
    fs::write(frontend.join("main.ts"), "export {};").unwrap();
    assert_eq!(
        diff(&frontend, "HEAD")["changed_source_files"],
        serde_json::json!(["main.ts"])
    );
    let before = git(root, &["worktree", "list", "--porcelain"]);
    let output = Command::new(env!("CARGO_BIN_EXE_oxarch"))
        .args(["diff", "missing-ref", root.to_str().unwrap(), "--json"])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert_eq!(git(root, &["worktree", "list", "--porcelain"]), before);
}

#[test]
fn inherited_aliases_resolve_relative_to_the_config_and_prefer_specific_paths() {
    let dir = tempdir().unwrap();
    let root = dir.path();
    fs::create_dir(root.join("config")).unwrap();
    fs::create_dir(root.join("source")).unwrap();
    fs::write(
        root.join("main.ts"),
        "import '@app/exact'; import '@app/other';",
    )
    .unwrap();
    fs::write(root.join("source/exact.ts"), "export {};").unwrap();
    fs::write(root.join("source/other.ts"), "export {};").unwrap();
    fs::write(root.join("source/fallback.ts"), "export {};").unwrap();
    fs::write(
        root.join("config/tsconfig.base.json"),
        r#"{
      // Shared configuration
      "compilerOptions": {"baseUrl": "../source", "paths": {
        "@app/*": ["fallback"], "@app/exact": ["exact"], "@app/other": ["other"],
      }}
    }"#,
    )
    .unwrap();
    fs::write(
        root.join("tsconfig.json"),
        r#"{"extends":"./config/tsconfig.base"}"#,
    )
    .unwrap();
    let result = analyze(root, false);
    assert_eq!(
        result["analysis"]["edges"],
        serde_json::json!([
            {"from":"main.ts","to":"source/exact.ts"}, {"from":"main.ts","to":"source/other.ts"}
        ])
    );
}
