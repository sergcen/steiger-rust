use std::{fs, path::Path};

use steiger::{Config, LintOptions, lint};
use tempfile::tempdir;

fn write(root: &Path, relative: &str, content: &str) {
    let path = root.join(relative);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, content).unwrap();
}

#[test]
fn an_ignored_import_target_is_removed_from_rule_analysis() {
    let temp = tempdir().unwrap();
    write(
        temp.path(),
        "tsconfig.json",
        r#"{ "compilerOptions": { "baseUrl": ".", "paths": { "@/*": ["./src/*"] } } }"#,
    );
    write(
        temp.path(),
        "src/features/search/ui/search.ts",
        "import { app } from '@/app/index'; export const search = app",
    );
    write(
        temp.path(),
        "src/features/search/index.ts",
        "export * from './ui/search'",
    );
    write(temp.path(), "src/app/index.ts", "export const app = 1");

    let default_result = lint(
        &temp.path().join("src"),
        LintOptions {
            config: &Config::recommended(),
        },
    )
    .unwrap();
    assert_eq!(
        default_result
            .diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.rule_name == "fsd/forbidden-imports")
            .count(),
        1
    );

    let config_path = temp.path().join("steiger.json");
    write(
        temp.path(),
        "steiger.json",
        r#"[
          {
            "files": ["./src/app/**"],
            "rules": { "fsd/forbidden-imports": "off" }
          }
        ]"#,
    );
    let config = Config::load(&config_path).unwrap();
    let scoped_result = lint(&temp.path().join("src"), LintOptions { config: &config }).unwrap();
    assert!(
        scoped_result
            .diagnostics
            .iter()
            .all(|diagnostic| diagnostic.rule_name != "fsd/forbidden-imports")
    );
}

#[test]
fn scoped_off_files_do_not_contribute_to_structural_counts() {
    let temp = tempdir().unwrap();
    for index in 0..21 {
        write(
            temp.path(),
            &format!("src/features/slice-{index}/ui/view.ts"),
            "export const view = 1",
        );
        write(
            temp.path(),
            &format!("src/features/slice-{index}/index.ts"),
            "export * from './ui/view'",
        );
    }

    let default_result = lint(
        &temp.path().join("src"),
        LintOptions {
            config: &Config::recommended(),
        },
    )
    .unwrap();
    assert_eq!(
        default_result
            .diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.rule_name == "fsd/excessive-slicing")
            .count(),
        1
    );

    let config_path = temp.path().join("steiger.json");
    write(
        temp.path(),
        "steiger.json",
        r#"[
          {
            "files": ["./src/features/slice-20/**"],
            "rules": { "fsd/excessive-slicing": "off" }
          }
        ]"#,
    );
    let config = Config::load(&config_path).unwrap();
    let scoped_result = lint(&temp.path().join("src"), LintOptions { config: &config }).unwrap();
    assert!(
        scoped_result
            .diagnostics
            .iter()
            .all(|diagnostic| diagnostic.rule_name != "fsd/excessive-slicing")
    );
}
