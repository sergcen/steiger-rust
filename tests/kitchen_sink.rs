use std::{collections::BTreeMap, fs, path::Path};

use steiger::{Config, LintOptions, lint};
use tempfile::tempdir;

fn write(root: &Path, relative: &str, content: &str) {
    let path = root.join(relative);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, content).unwrap();
}

#[test]
fn matches_the_upstream_kitchen_sink_diagnostics() {
    let temp = tempdir().unwrap();
    write(
        temp.path(),
        "tsconfig.json",
        r#"{
          "compilerOptions": {
            "baseUrl": ".",
            "paths": { "@/*": ["./src/*"] },
            "moduleResolution": "Bundler"
          }
        }"#,
    );
    write(
        temp.path(),
        "src/entities/user/api/getUser.ts",
        "import { App } from '@/app/ui/App'\nexport const getUser = () => App",
    );
    write(
        temp.path(),
        "src/entities/user/ui/UserInfo.tsx",
        "export const UserInfo = 1",
    );
    write(
        temp.path(),
        "src/entities/user/ui/api/method.ts",
        "export const method = 1",
    );
    write(
        temp.path(),
        "src/entities/user/index.ts",
        "export * from './ui/UserInfo'",
    );
    write(
        temp.path(),
        "src/entities/users/api/getUsers.ts",
        "export const getUsers = () => []",
    );
    write(
        temp.path(),
        "src/entities/users/ui/UserTable.tsx",
        "export const UserTable = 1",
    );
    write(
        temp.path(),
        "src/entities/users/index.ts",
        "export * from './ui/UserTable'",
    );
    write(temp.path(), "src/app/ui/App.tsx", "export const App = 1");
    write(temp.path(), "src/app/ui/index.ts", "export * from './App'");
    write(
        temp.path(),
        "src/processes/auth/index.ts",
        "export const auth = 1",
    );

    let result = lint(
        &temp.path().join("src"),
        LintOptions {
            config: &Config::recommended(),
        },
    )
    .unwrap();

    let counts = result
        .diagnostics
        .iter()
        .fold(BTreeMap::new(), |mut counts, diagnostic| {
            *counts
                .entry(diagnostic.rule_name.as_str())
                .or_insert(0_usize) += 1;
            counts
        });
    assert_eq!(result.diagnostics.len(), 8);
    assert_eq!(counts["fsd/forbidden-imports"], 1);
    assert_eq!(counts["fsd/inconsistent-naming"], 1);
    assert_eq!(counts["fsd/insignificant-slice"], 2);
    assert_eq!(counts["fsd/no-processes"], 1);
    assert_eq!(counts["fsd/no-public-api-sidestep"], 1);
    assert_eq!(counts["fsd/no-reserved-folder-names"], 1);
    assert_eq!(counts["fsd/no-ui-in-app"], 1);
    assert_eq!(
        result
            .diagnostics
            .iter()
            .map(|diagnostic| diagnostic.rule_name.as_str())
            .collect::<Vec<_>>(),
        vec![
            "fsd/forbidden-imports",
            "fsd/inconsistent-naming",
            "fsd/insignificant-slice",
            "fsd/insignificant-slice",
            "fsd/no-public-api-sidestep",
            "fsd/no-reserved-folder-names",
            "fsd/no-ui-in-app",
            "fsd/no-processes",
        ]
    );
}
