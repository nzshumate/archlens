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
    assert!(root.join("target/oxarch/parsed-v2.json").is_file());
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
        serde_json::from_slice(&fs::read(root.join("target/oxarch/parsed-v2.json")).unwrap())
            .unwrap();
    assert!(cache["entries"].get("a.ts").is_none());
    fs::write(root.join("target/oxarch/parsed-v2.json"), "broken").unwrap();
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

fn write(root: &std::path::Path, name: &str, contents: &str) {
    let path = root.join(name);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, contents).unwrap();
}
fn check_json(root: &std::path::Path, strict: bool) -> (bool, serde_json::Value) {
    let mut command = Command::new(env!("CARGO_BIN_EXE_oxarch"));
    command.args([
        "check",
        root.to_str().unwrap(),
        "--json",
        "--min-health",
        "0",
        "--allow-cycles",
    ]);
    if strict {
        command.arg("--strict");
    }
    let output = command.output().unwrap();
    let value = serde_json::from_slice(&output.stdout)
        .unwrap_or_else(|_| panic!("{}", String::from_utf8_lossy(&output.stderr)));
    (output.status.success(), value)
}
#[test]
fn discovery_respects_project_ignore_files_and_negation_without_git() {
    let dir = tempdir().unwrap();
    let root = dir.path();
    write(root, ".gitignore", "generated/*\n!generated/keep.ts\n");
    write(root, ".oxarchignore", "**/*.test.ts\n");
    for name in [
        "main.ts",
        "main.test.ts",
        "generated/drop.ts",
        "generated/keep.ts",
        "src/.hidden.ts",
        "node_modules/library.ts",
    ] {
        write(root, name, "export {};");
    }
    let result = analyze(root, false);
    assert_eq!(
        result["analysis"]["nodes"],
        serde_json::json!(["generated/keep.ts", "main.ts", "src/.hidden.ts"])
    );
    // Ignore-file edits must take effect even when a parser cache exists.
    write(root, ".oxarchignore", "**/*.test.ts\nsrc/\n");
    assert_eq!(analyze(root, false)["analysis"]["source_files"], 2);
}
#[test]
fn nearest_tsconfig_and_jsconfig_resolve_independently_in_a_monorepo() {
    let dir = tempdir().unwrap();
    let root = dir.path();
    write(
        root,
        "tsconfig.json",
        r#"{"compilerOptions":{"paths":{"@/*":["root/*"]}}}"#,
    );
    write(root, "root/value.ts", "export {};");
    write(root, "main.ts", "import '@/value';");
    write(
        root,
        "packages/a/tsconfig.json",
        r#"{"compilerOptions":{"paths":{"@/*.js":["src/*.ts"]}}}"#,
    );
    write(root, "packages/a/main.ts", "import '@/value.js';");
    write(root, "packages/a/src/value.ts", "export {};");
    write(
        root,
        "packages/b/jsconfig.json",
        r#"{"compilerOptions":{"baseUrl":"./src"}}"#,
    );
    write(root, "packages/b/main.js", "import 'value';");
    write(root, "packages/b/src/value.js", "export {};");
    let result = analyze(root, false);
    assert_eq!(
        result["analysis"]["edges"],
        serde_json::json!([
            {"from":"main.ts","to":"root/value.ts"},
            {"from":"packages/a/main.ts","to":"packages/a/src/value.ts"},
            {"from":"packages/b/main.js","to":"packages/b/src/value.js"}
        ])
    );
    assert_eq!(result["analysis"]["diagnostics"], serde_json::json!([]));
}
#[test]
fn modern_typescript_extensions_follow_runtime_extension_substitution() {
    let dir = tempdir().unwrap();
    let root = dir.path();
    write(
        root,
        "main.ts",
        "import './esm.mjs'; import './common.cjs'; import './view.jsx';",
    );
    for file in [
        "esm.mts",
        "esm.mjs",
        "esm.ts",
        "common.cts",
        "common.cjs",
        "view.tsx",
        "view.jsx",
    ] {
        write(root, file, "export {};");
    }
    let result = analyze(root, false);
    assert_eq!(result["analysis"]["dependencies"], 3);
    let targets = result["analysis"]["edges"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["to"].as_str().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(targets, vec!["common.cts", "esm.mts", "view.tsx"]);
}
#[test]
fn workspace_exports_source_and_deep_packages_are_resolved() {
    let dir = tempdir().unwrap();
    let root = dir.path();
    write(root,"main.ts","import '@scope/ui'; import '@scope/ui/components/button'; import '@scope/ui/private'; import 'data';");
    let package = "packages/deep/group/ui";
    write(
        root,
        &format!("{package}/package.json"),
        r#"{"name":"@scope/ui","exports":{".":{"source":"./src/index.ts","default":"./dist/index.js"},"./components/*":"./src/components/*.ts"}}"#,
    );
    write(root, &format!("{package}/src/index.ts"), "export {};");
    write(
        root,
        &format!("{package}/src/components/button.ts"),
        "export {};",
    );
    write(root, &format!("{package}/src/private.ts"), "export {};");
    write(
        root,
        "packages/data/package.json",
        r#"{"name":"data","source":"./entry.ts","main":"./dist/index.js"}"#,
    );
    write(root, "packages/data/entry.ts", "export {};");
    let result = analyze(root, false);
    assert_eq!(result["analysis"]["dependencies"], 3);
    let diagnostics = result["analysis"]["diagnostics"].as_array().unwrap();
    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0]["message"]
        .as_str()
        .unwrap()
        .contains("@scope/ui/private"));
}
#[test]
fn explicit_entrypoints_find_unreachable_cycles_and_support_multiple_roots() {
    let dir = tempdir().unwrap();
    let root = dir.path();
    write(root, "main.ts", "import './reachable';");
    write(root, "reachable.ts", "export {};");
    write(root, "dead-a.ts", "import './dead-b';");
    write(root, "dead-b.ts", "import './dead-a';");
    write(root, "oxarch.json", r#"{"entryPoints":["./main.ts"]}"#);
    let result = analyze(root, false);
    assert_eq!(result["metrics"]["reachability_mode"], "explicit");
    assert_eq!(
        result["metrics"]["dead_candidates"],
        serde_json::json!(["dead-a.ts", "dead-b.ts"])
    );
    write(
        root,
        "oxarch.json",
        r#"{"entryPoints":["main.ts","dead-a.ts"]}"#,
    );
    assert_eq!(
        analyze(root, false)["metrics"]["dead_candidates"],
        serde_json::json!([])
    );
}
#[test]
fn diagnostics_survive_cache_and_check_json_fails_for_parse_errors() {
    let dir = tempdir().unwrap();
    let root = dir.path();
    write(root, "main.ts", "export const ok = 1;\nconst broken = ;\n");
    let cold = analyze(root, false);
    let warm = analyze(root, false);
    assert_eq!(cold, warm);
    assert_eq!(cold, analyze(root, true));
    assert_eq!(cold["analysis"]["diagnostics"][0]["code"], "parse_error");
    assert_eq!(cold["analysis"]["diagnostics"][0]["line"], 2);
    let (passed, result) = check_json(root, false);
    assert!(!passed);
    assert_eq!(result["check"]["passed"], false);
    assert!(result["check"]["failures"]
        .as_array()
        .unwrap()
        .contains(&serde_json::json!("parse_errors")));
}
#[test]
fn strict_checks_report_missing_internal_imports_without_flagging_external_packages_or_assets() {
    let dir = tempdir().unwrap();
    let root = dir.path();
    write(
        root,
        "main.ts",
        "import './missing'; import '@missing?raw'; import 'react'; import './styles.css'; import './icon.svg?url';",
    );
    write(
        root,
        "tsconfig.json",
        r#"{"compilerOptions":{"paths":{"@missing":["./missing"]}}}"#,
    );
    let result = analyze(root, false);
    assert_eq!(
        result["analysis"]["diagnostics"].as_array().unwrap().len(),
        2
    );
    assert!(check_json(root, false).0);
    assert!(!check_json(root, true).0);
    write(root, "missing.ts", "export {};");
    assert!(check_json(root, true).0);
}
#[test]
fn vue_external_scripts_and_diagnostic_locations_are_supported() {
    let dir = tempdir().unwrap();
    let root = dir.path();
    write(root,"App.vue","<template><p>Hello</p></template>\n<script src='./logic.ts'></script>\n<script setup lang='ts'>\nconst broken = ;\n</script>");
    write(root, "logic.ts", "export {};");
    let result = analyze(root, false);
    assert_eq!(result["analysis"]["dependencies"], 1);
    assert_eq!(result["analysis"]["diagnostics"][0]["line"], 4);
}
#[test]
fn malformed_circular_and_missing_configurations_are_actionable_errors() {
    for (config, base, expected) in [
        ("{", None, "invalid TypeScript configuration"),
        (
            r#"{"extends":"./base"}"#,
            Some(r#"{"extends":"./tsconfig"}"#),
            "circular TypeScript configuration",
        ),
        (
            r#"{"extends":"./missing"}"#,
            None,
            "cannot open TypeScript configuration",
        ),
        (
            "{} /* unterminated",
            None,
            "invalid TypeScript configuration",
        ),
    ] {
        let dir = tempdir().unwrap();
        let root = dir.path();
        write(root, "main.ts", "export {};");
        write(root, "tsconfig.json", config);
        if let Some(base) = base {
            write(root, "base.json", base);
        }
        let output = Command::new(env!("CARGO_BIN_EXE_oxarch"))
            .args(["analyze", root.to_str().unwrap(), "--json"])
            .output()
            .unwrap();
        assert!(!output.status.success());
        assert!(
            String::from_utf8_lossy(&output.stderr).contains(expected),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}
#[test]
fn empty_projects_and_invalid_entrypoints_cannot_pass_ci_silently() {
    let dir = tempdir().unwrap();
    let root = dir.path();
    assert!(!check_json(root, false).0);
    write(root, "main.ts", "export {};");
    write(root, "oxarch.json", r#"{"entryPoints":["missing.ts"]}"#);
    let output = Command::new(env!("CARGO_BIN_EXE_oxarch"))
        .args(["check", root.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("missing, ignored"));
    write(root, "oxarch.json", r#"{"boundries":[]}"#);
    let output = Command::new(env!("CARGO_BIN_EXE_oxarch"))
        .args(["check", root.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(!output.status.success());
}
#[test]
fn diff_preserves_analysis_diagnostics_in_both_snapshots() {
    let dir = tempdir().unwrap();
    let root = dir.path();
    init(root);
    write(root, "main.ts", "import './missing';");
    commit(root);
    write(root, "main.ts", "import './new-missing';");
    let result = diff(root, "HEAD");
    assert_eq!(result["analysis_complete"], false);
    assert_eq!(result["base_diagnostics"].as_array().unwrap().len(), 1);
    assert_eq!(result["current_diagnostics"].as_array().unwrap().len(), 1);
}
