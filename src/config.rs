use std::{
    collections::BTreeMap,
    env, fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, bail};
use globset::{GlobBuilder, GlobMatcher};
use serde::Deserialize;

use crate::{diagnostic::Severity, rules::RECOMMENDED_RULES};

const CONFIG_NAMES: &[&str] = &[
    "steiger.toml",
    ".steiger.toml",
    "steiger.config.toml",
    "steiger.json",
    ".steiger.json",
    "steiger.config.json",
];

#[derive(Debug)]
pub struct Config {
    groups: Vec<RuleGroup>,
    global_ignores: Vec<Pattern>,
    pub path: Option<PathBuf>,
}

#[derive(Debug)]
struct RuleGroup {
    files: Option<Vec<Pattern>>,
    ignores: Vec<Pattern>,
    rules: BTreeMap<String, Severity>,
}

#[derive(Debug)]
struct Pattern {
    negated: bool,
    matcher: GlobMatcher,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum RuleSetting {
    Severity(SeverityValue),
    WithOptions((SeverityValue, serde_json::Value)),
}

impl RuleSetting {
    fn severity(&self) -> Severity {
        let value = match self {
            Self::Severity(value) | Self::WithOptions((value, _)) => value,
        };
        (*value).into()
    }
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "lowercase")]
enum SeverityValue {
    Off,
    Warn,
    Error,
}

impl From<SeverityValue> for Severity {
    fn from(value: SeverityValue) -> Self {
        match value {
            SeverityValue::Off => Self::Off,
            SeverityValue::Warn => Self::Warn,
            SeverityValue::Error => Self::Error,
        }
    }
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct StructuredConfig {
    #[serde(default, alias = "globalIgnores")]
    global_ignores: Vec<String>,
    #[serde(default)]
    rules: BTreeMap<String, RuleSetting>,
    #[serde(default)]
    overrides: Vec<RawRuleGroup>,
}

#[derive(Debug, Default, Deserialize)]
struct RawRuleGroup {
    files: Option<Vec<String>>,
    #[serde(default)]
    ignores: Vec<String>,
    #[serde(default)]
    rules: BTreeMap<String, RuleSetting>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum ConfigDocument {
    Structured(StructuredConfig),
    Flat(Vec<RawRuleGroup>),
}

impl Default for Config {
    fn default() -> Self {
        Self::recommended()
    }
}

impl Config {
    pub fn recommended() -> Self {
        let rules = RECOMMENDED_RULES
            .iter()
            .map(|rule| ((*rule).to_owned(), Severity::Error))
            .collect();
        Self {
            groups: vec![RuleGroup {
                files: None,
                ignores: Vec::new(),
                rules,
            }],
            global_ignores: Vec::new(),
            path: None,
        }
    }

    pub fn discover(explicit_path: Option<&Path>) -> Result<Self> {
        let path = if let Some(path) = explicit_path {
            Some(path.canonicalize().with_context(|| {
                format!("configuration file does not exist: {}", path.display())
            })?)
        } else {
            discover_config(&env::current_dir().context("cannot determine current directory")?)
        };

        match path {
            Some(path) => Self::load(&path),
            None => Ok(Self::recommended()),
        }
    }

    pub fn load(path: &Path) -> Result<Self> {
        let path = path
            .canonicalize()
            .with_context(|| format!("configuration file does not exist: {}", path.display()))?;
        let source = fs::read_to_string(&path)
            .with_context(|| format!("cannot read configuration from {}", path.display()))?;
        let extension = path.extension().and_then(|value| value.to_str());
        let document: ConfigDocument = match extension {
            Some("json") => serde_json::from_str(&source)
                .with_context(|| format!("invalid JSON configuration in {}", path.display()))?,
            Some("toml") => toml::from_str(&source)
                .with_context(|| format!("invalid TOML configuration in {}", path.display()))?,
            _ => bail!("configuration must be a .toml or .json file"),
        };

        let base = path.parent().unwrap_or_else(|| Path::new("."));
        let mut config = Self::recommended();
        config.path = Some(path.clone());

        match document {
            ConfigDocument::Structured(raw) => {
                config.global_ignores = compile_patterns(&raw.global_ignores, base)?;
                if !raw.rules.is_empty() {
                    config.groups.push(compile_group(
                        RawRuleGroup {
                            rules: raw.rules,
                            ..RawRuleGroup::default()
                        },
                        base,
                    )?);
                }
                for group in raw.overrides {
                    config.groups.push(compile_group(group, base)?);
                }
            }
            ConfigDocument::Flat(items) => {
                for item in items {
                    if item.rules.is_empty() && item.files.is_none() {
                        config
                            .global_ignores
                            .extend(compile_patterns(&item.ignores, base)?);
                    } else {
                        config.groups.push(compile_group(item, base)?);
                    }
                }
            }
        }

        config.validate_rule_names()?;
        Ok(config)
    }

    pub fn is_global_ignored(&self, path: &Path) -> bool {
        matches_patterns(path, &self.global_ignores)
    }

    pub fn severity_for_file(&self, rule: &str, path: &Path) -> Severity {
        let mut severity = Severity::Off;
        for group in &self.groups {
            let Some(configured) = group.rules.get(rule) else {
                continue;
            };
            if group.matches(path) {
                severity = *configured;
            }
        }
        severity
    }

    pub fn severity_for_location(
        &self,
        rule: &str,
        location: &Path,
        project_files: &[PathBuf],
    ) -> Severity {
        if project_files
            .binary_search_by(|file| file.as_path().cmp(location))
            .is_ok()
            || location.is_file()
        {
            return self.severity_for_file(rule, location);
        }

        let descendant_prefix = location.join("");
        let first_descendant =
            project_files.partition_point(|file| file.as_path() < descendant_prefix.as_path());
        project_files[first_descendant..]
            .iter()
            .take_while(|file| file.starts_with(location))
            .map(|file| self.severity_for_file(rule, file))
            .max()
            .unwrap_or_else(|| self.severity_for_file(rule, location))
    }

    fn validate_rule_names(&self) -> Result<()> {
        for rule in self.groups.iter().flat_map(|group| group.rules.keys()) {
            if !crate::rules::ALL_RULES.contains(&rule.as_str()) {
                bail!("unknown rule in configuration: {rule}");
            }
        }
        Ok(())
    }
}

impl RuleGroup {
    fn matches(&self, path: &Path) -> bool {
        let included = self.files.as_ref().is_none_or(|patterns| {
            !patterns.is_empty() && patterns.iter().any(|p| p.matcher.is_match(path))
        });
        included && !matches_patterns(path, &self.ignores)
    }
}

fn discover_config(start: &Path) -> Option<PathBuf> {
    for directory in start.ancestors() {
        for name in CONFIG_NAMES {
            let candidate = directory.join(name);
            if candidate.is_file() {
                return candidate.canonicalize().ok();
            }
        }
    }
    None
}

fn compile_group(raw: RawRuleGroup, base: &Path) -> Result<RuleGroup> {
    Ok(RuleGroup {
        files: raw
            .files
            .as_ref()
            .map(|patterns| compile_patterns(patterns, base))
            .transpose()?,
        ignores: compile_patterns(&raw.ignores, base)?,
        rules: raw
            .rules
            .into_iter()
            .map(|(name, setting)| (name, setting.severity()))
            .collect(),
    })
}

fn compile_patterns(raw: &[String], base: &Path) -> Result<Vec<Pattern>> {
    raw.iter()
        .map(|pattern| compile_pattern(pattern, base))
        .collect()
}

fn compile_pattern(raw: &str, base: &Path) -> Result<Pattern> {
    let (negated, glob) = raw
        .strip_prefix('!')
        .map_or((false, raw), |glob| (true, glob));
    let glob = glob
        .strip_prefix("./")
        .or_else(|| glob.strip_prefix(".\\"))
        .unwrap_or(glob);
    let glob_path = Path::new(glob);
    let absolute = if glob_path.is_absolute() {
        glob_path.to_owned()
    } else {
        base.join(glob_path)
    };
    let normalized = absolute.to_string_lossy().replace('\\', "/");
    let matcher = GlobBuilder::new(normalized.trim_end_matches('/'))
        .literal_separator(true)
        .backslash_escape(false)
        .build()
        .with_context(|| format!("invalid glob pattern: {raw}"))?
        .compile_matcher();
    Ok(Pattern { negated, matcher })
}

fn matches_patterns(path: &Path, patterns: &[Pattern]) -> bool {
    let mut ignored = patterns
        .iter()
        .filter(|pattern| !pattern.negated)
        .any(|pattern| pattern.matcher.is_match(path));
    if ignored
        && patterns
            .iter()
            .filter(|pattern| pattern.negated)
            .any(|pattern| pattern.matcher.is_match(path))
    {
        ignored = false;
    }
    ignored
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::tempdir;

    use super::*;

    #[test]
    fn flat_config_overrides_recommended_severity() {
        let temp = tempdir().unwrap();
        let config_path = temp.path().join("steiger.json");
        fs::write(
            &config_path,
            r#"[
              { "files": ["./src/shared/**"], "rules": { "fsd/public-api": "off" } },
              { "files": ["./src/shared/ui/**"], "rules": { "fsd/public-api": "warn" } }
            ]"#,
        )
        .unwrap();

        let config = Config::load(&config_path).unwrap();
        let root = temp.path().canonicalize().unwrap();
        assert_eq!(
            config.severity_for_file("fsd/public-api", &root.join("src/shared/api/a.ts")),
            Severity::Off
        );
        assert_eq!(
            config.severity_for_file("fsd/public-api", &root.join("src/shared/ui/a.ts")),
            Severity::Warn
        );
    }

    #[test]
    fn loads_structured_toml_and_global_ignores() {
        let temp = tempdir().unwrap();
        let config_path = temp.path().join("steiger.toml");
        fs::write(
            &config_path,
            r#"
              global_ignores = ["**/generated/**"]

              [rules]
              "fsd/no-processes" = "warn"

              [[overrides]]
              files = ["./src/shared/**"]

              [overrides.rules]
              "fsd/public-api" = "off"
            "#,
        )
        .unwrap();

        let config = Config::load(&config_path).unwrap();
        let root = temp.path().canonicalize().unwrap();
        assert!(config.is_global_ignored(&root.join("src/generated/code.ts")));
        assert_eq!(
            config.severity_for_file("fsd/no-processes", &root.join("src/processes/auth.ts")),
            Severity::Warn
        );
        assert_eq!(
            config.severity_for_file("fsd/public-api", &root.join("src/shared/api/client.ts")),
            Severity::Off
        );
    }

    #[test]
    fn folder_severity_uses_descendants_not_textual_prefix_siblings() {
        let temp = tempdir().unwrap();
        let config_path = temp.path().join("steiger.json");
        fs::write(
            &config_path,
            r#"{
              "rules": { "fsd/public-api": "off" },
              "overrides": [{
                "files": ["./src/entities/product/ui/**"],
                "rules": { "fsd/public-api": "warn" }
              }]
            }"#,
        )
        .unwrap();

        let config = Config::load(&config_path).unwrap();
        let root = temp.path().canonicalize().unwrap();
        let ui = root.join("src/entities/product/ui");
        let mut files = vec![
            ui.join("Card.tsx"),
            root.join("src/entities/product/ui-kit/theme.ts"),
        ];
        files.sort();

        assert_eq!(
            config.severity_for_location("fsd/public-api", &ui, &files),
            Severity::Warn
        );
    }
}
