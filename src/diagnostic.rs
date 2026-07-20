use std::path::PathBuf;

use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Off,
    Warn,
    Error,
}

impl Severity {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Off => "off",
            Self::Warn => "warn",
            Self::Error => "error",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Location {
    pub path: PathBuf,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub column: Option<u32>,
}

impl Location {
    pub fn at(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            line: None,
            column: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum Fix {
    Rename {
        path: PathBuf,
        #[serde(rename = "newName")]
        new_name: String,
    },
    CreateFile {
        path: PathBuf,
        content: String,
    },
    CreateFolder {
        path: PathBuf,
    },
    Delete {
        path: PathBuf,
    },
    ModifyFile {
        path: PathBuf,
        content: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Diagnostic {
    pub message: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub fixes: Vec<Fix>,
    pub location: Location,
    pub rule_name: String,
    pub severity: Severity,
}

impl Diagnostic {
    pub fn new(
        rule_name: &'static str,
        message: impl Into<String>,
        path: impl Into<PathBuf>,
    ) -> Self {
        Self {
            message: message.into(),
            fixes: Vec::new(),
            location: Location::at(path),
            rule_name: rule_name.to_owned(),
            severity: Severity::Error,
        }
    }

    pub fn with_fixes(mut self, fixes: Vec<Fix>) -> Self {
        self.fixes = fixes;
        self
    }

    pub fn description_url(&self) -> String {
        let rule = self
            .rule_name
            .split('/')
            .next_back()
            .unwrap_or(&self.rule_name);
        format!(
            "https://github.com/feature-sliced/steiger/tree/master/packages/steiger-plugin-fsd/src/{rule}"
        )
    }
}
