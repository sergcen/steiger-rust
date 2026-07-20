use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::Path,
    process::{Command, Output},
};

use steiger::{Config, LintOptions, lint};
use tempfile::tempdir;

fn run_steiger(project: &Path, flag: &str, baseline: &Path) -> Output {
    Command::new(env!("CARGO_BIN_EXE_steiger"))
        .current_dir(project)
        .arg(project.join("src"))
        .arg(flag)
        .arg(baseline)
        .arg("--reporter")
        .arg("json")
        .output()
        .unwrap()
}

#[test]
fn exits_only_when_a_diagnostic_is_absent_from_the_baseline() {
    let temp = tempdir().unwrap();
    fs::create_dir_all(temp.path().join("src/processes/checkout")).unwrap();
    fs::write(
        temp.path().join("src/processes/checkout/index.ts"),
        "export {};",
    )
    .unwrap();
    let project = temp.path().canonicalize().unwrap();
    let root = project.join("src");
    let diagnostics = lint(
        &root,
        LintOptions {
            config: &Config::recommended(),
        },
    )
    .unwrap()
    .diagnostics;
    assert!(!diagnostics.is_empty());

    let mut grouped: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for diagnostic in &diagnostics {
        grouped
            .entry(diagnostic.rule_name.clone())
            .or_default()
            .insert(
                diagnostic
                    .location
                    .path
                    .to_string_lossy()
                    .replace('\\', "/"),
            );
    }
    let baseline = project.join("fsd-errors.json");
    fs::write(&baseline, serde_json::to_string_pretty(&grouped).unwrap()).unwrap();

    let accepted = run_steiger(&project, "--baseline", &baseline);
    assert_eq!(accepted.status.code(), Some(0));
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&accepted.stdout).unwrap(),
        serde_json::json!([])
    );

    fs::write(&baseline, "{}").unwrap();
    let rejected = run_steiger(&project, "--fsd-errors", &baseline);
    assert_eq!(rejected.status.code(), Some(1));
    assert_eq!(
        serde_json::from_slice::<Vec<serde_json::Value>>(&rejected.stdout)
            .unwrap()
            .len(),
        diagnostics.len()
    );
    assert!(String::from_utf8_lossy(&rejected.stderr).contains("new diagnostic"));
}
