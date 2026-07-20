use std::{cmp::Ordering, fs, path::Path, time::Duration};

use anyhow::{Context, Result};

use crate::{
    Config,
    diagnostic::{Diagnostic, Fix, Severity},
    fsd::Project,
    imports::ImportGraph,
    rules,
};

#[derive(Debug)]
pub struct LintOptions<'a> {
    pub config: &'a Config,
}

#[derive(Debug)]
pub struct LintResult {
    pub diagnostics: Vec<Diagnostic>,
    pub files_scanned: usize,
    pub elapsed: Duration,
}

pub fn lint(root: &Path, options: LintOptions<'_>) -> Result<LintResult> {
    let started = std::time::Instant::now();
    let project = Project::scan(root, options.config)?;
    let source_index = project.source_index();
    let graph = ImportGraph::build(&project, &source_index)?;
    let import_analysis = rules::ImportAnalysis::new(&graph, &source_index);
    let mut raw_diagnostics = Vec::new();
    for rule in rules::ALL_RULES {
        let mut filtered_files = None;
        for (index, file) in project.files.iter().enumerate() {
            if options.config.severity_for_file(rule, file) == Severity::Off {
                if filtered_files.is_none() {
                    filtered_files = Some(project.files[..index].to_vec());
                }
            } else if let Some(files) = &mut filtered_files {
                files.push(file.clone());
            }
        }
        let scoped_project;
        let rule_project = match filtered_files {
            None => &project,
            Some(files) if files.is_empty() => continue,
            Some(files) => {
                scoped_project = project.with_files(files);
                &scoped_project
            }
        };
        let scoped_source_index;
        let scoped_import_analysis;
        let rule_import_analysis =
            if rule_project.files != project.files && rules::is_import_rule(rule) {
                scoped_source_index = rule_project.source_index();
                scoped_import_analysis = rules::ImportAnalysis::new(&graph, &scoped_source_index);
                &scoped_import_analysis
            } else {
                &import_analysis
            };
        let mut rule_diagnostics = rules::run_rule(rule, rule_project, rule_import_analysis);
        rule_diagnostics.sort_by(compare_diagnostic_paths);
        raw_diagnostics.extend(rule_diagnostics);
    }
    let diagnostics = raw_diagnostics
        .into_iter()
        .filter_map(|mut diagnostic| {
            let severity = options.config.severity_for_location(
                &diagnostic.rule_name,
                &diagnostic.location.path,
                &project.files,
            );
            (severity != Severity::Off).then(|| {
                diagnostic.severity = severity;
                diagnostic
            })
        })
        .collect::<Vec<_>>();
    Ok(LintResult {
        diagnostics,
        files_scanned: project.files.len(),
        elapsed: started.elapsed(),
    })
}

fn compare_diagnostic_paths(left: &Diagnostic, right: &Diagnostic) -> Ordering {
    let left_folded = left.location.path.to_string_lossy().to_lowercase();
    let right_folded = right.location.path.to_string_lossy().to_lowercase();
    left_folded
        .cmp(&right_folded)
        .then_with(|| left.location.path.cmp(&right.location.path))
}

pub fn apply_fixes(diagnostics: &[Diagnostic]) -> Result<Vec<Diagnostic>> {
    let mut remaining = Vec::new();
    for diagnostic in diagnostics {
        if diagnostic.fixes.is_empty() {
            remaining.push(diagnostic.clone());
            continue;
        }
        for fix in &diagnostic.fixes {
            apply_fix(fix).with_context(|| {
                format!(
                    "could not apply fix for {} at {}",
                    diagnostic.rule_name,
                    diagnostic.location.path.display()
                )
            })?;
        }
    }
    Ok(remaining)
}

fn apply_fix(fix: &Fix) -> Result<()> {
    match fix {
        Fix::Rename { path, new_name } => {
            let destination = path
                .parent()
                .unwrap_or_else(|| Path::new("."))
                .join(new_name);
            fs::rename(path, destination)?;
        }
        Fix::CreateFile { path, content } | Fix::ModifyFile { path, content } => {
            fs::write(path, content)?;
        }
        Fix::CreateFolder { path } => {
            fs::create_dir_all(path)?;
        }
        Fix::Delete { path } if path.is_dir() => {
            fs::remove_dir_all(path)?;
        }
        Fix::Delete { path } => {
            fs::remove_file(path)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;

    use super::*;
    use crate::diagnostic::Diagnostic;

    #[test]
    fn applies_create_file_and_rename_fixes() {
        let temp = tempdir().unwrap();
        let slice = temp.path().join("user");
        fs::create_dir(&slice).unwrap();
        let diagnostics = vec![
            Diagnostic::new("fsd/public-api", "missing", &slice).with_fixes(vec![
                Fix::CreateFile {
                    path: slice.join("index.js"),
                    content: "export {}".to_owned(),
                },
            ]),
            Diagnostic::new("fsd/inconsistent-naming", "rename", &slice).with_fixes(vec![
                Fix::Rename {
                    path: slice.clone(),
                    new_name: "users".to_owned(),
                },
            ]),
        ];

        assert!(apply_fixes(&diagnostics).unwrap().is_empty());
        assert_eq!(
            fs::read_to_string(temp.path().join("users/index.js")).unwrap(),
            "export {}"
        );
    }

    #[test]
    fn sorts_ascii_paths_like_javascript_locale_compare() {
        let mut diagnostics = vec![
            Diagnostic::new(
                "fsd/test",
                "account tariff",
                "/src/pages/AccountTariff/index.ts",
            ),
            Diagnostic::new(
                "fsd/test",
                "accounts reports",
                "/src/pages/AccountsReports/index.ts",
            ),
        ];

        diagnostics.sort_by(compare_diagnostic_paths);

        assert_eq!(diagnostics[0].message, "accounts reports");
        assert_eq!(diagnostics[1].message, "account tariff");
    }
}
