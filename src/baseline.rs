use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};

use crate::{Diagnostic, path_utils::normalize_canonical_path};

/// A checked-in set of diagnostics that are allowed while existing FSD debt is reduced.
#[derive(Debug)]
pub struct DiagnosticBaseline {
    base_directory: PathBuf,
    entries: BTreeMap<String, BTreeSet<String>>,
}

impl DiagnosticBaseline {
    pub fn load(path: &Path) -> Result<Self> {
        let path = normalize_canonical_path(
            path.canonicalize()
                .with_context(|| format!("cannot resolve baseline file {}", path.display()))?,
        );
        let contents = fs::read_to_string(&path)
            .with_context(|| format!("cannot read baseline file {}", path.display()))?;
        let raw: BTreeMap<String, Vec<String>> = serde_json::from_str(&contents)
            .with_context(|| format!("invalid baseline file {}", path.display()))?;
        let entries = raw
            .into_iter()
            .map(|(rule, paths)| {
                (
                    rule,
                    paths
                        .into_iter()
                        .map(|path| normalize_path(&path))
                        .collect(),
                )
            })
            .collect();

        Ok(Self {
            base_directory: path
                .parent()
                .expect("a canonical file path has a parent")
                .to_owned(),
            entries,
        })
    }

    pub fn contains(&self, diagnostic: &Diagnostic) -> bool {
        let Some(allowed_paths) = self.entries.get(&diagnostic.rule_name) else {
            return false;
        };
        let absolute = normalize_path(&diagnostic.location.path.to_string_lossy());
        if allowed_paths.contains(&absolute) {
            return true;
        }

        diagnostic
            .location
            .path
            .strip_prefix(&self.base_directory)
            .ok()
            .map(|path| normalize_path(&path.to_string_lossy()))
            .is_some_and(|path| allowed_paths.contains(&path))
    }

    pub fn retain_new(&self, diagnostics: Vec<Diagnostic>) -> Vec<Diagnostic> {
        diagnostics
            .into_iter()
            .filter(|diagnostic| !self.contains(diagnostic))
            .collect()
    }
}

fn normalize_path(path: &str) -> String {
    let normalized = path.replace('\\', "/");
    normalized
        .strip_prefix("./")
        .unwrap_or(&normalized)
        .trim_end_matches('/')
        .to_owned()
}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;

    use super::*;

    #[test]
    fn matches_rule_and_path_relative_to_the_baseline_file() {
        let temp = tempdir().unwrap();
        let baseline_path = temp.path().join("fsd-errors.json");
        fs::create_dir_all(temp.path().join("src/entities/user")).unwrap();
        fs::write(
            &baseline_path,
            r#"{"fsd/insignificant-slice":["src/entities/user"]}"#,
        )
        .unwrap();
        let baseline = DiagnosticBaseline::load(&baseline_path).unwrap();
        let project = normalize_canonical_path(temp.path().canonicalize().unwrap());

        let known = Diagnostic::new(
            "fsd/insignificant-slice",
            "known",
            project.join("src/entities/user"),
        );
        let new_path = Diagnostic::new(
            "fsd/insignificant-slice",
            "new",
            project.join("src/entities/account"),
        );
        let new_rule = Diagnostic::new(
            "fsd/no-processes",
            "new rule",
            project.join("src/processes"),
        );

        assert!(baseline.contains(&known));
        assert!(!baseline.contains(&new_path));
        assert!(!baseline.contains(&new_rule));
    }

    #[test]
    fn accepts_absolute_paths_and_windows_separators() {
        let temp = tempdir().unwrap();
        let target = temp.path().join("src/features/search");
        let baseline_path = temp.path().join("fsd-errors.json");
        let windows_path = target.to_string_lossy().replace('/', "\\");
        fs::write(
            &baseline_path,
            serde_json::json!({ "fsd/test": [windows_path] }).to_string(),
        )
        .unwrap();
        let baseline = DiagnosticBaseline::load(&baseline_path).unwrap();

        assert!(baseline.contains(&Diagnostic::new("fsd/test", "known", target)));
    }
}
