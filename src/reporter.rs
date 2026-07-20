use std::path::Path;

use anyhow::Result;
use owo_colors::{OwoColorize, Stream};

use crate::{Diagnostic, Severity};

pub fn json(diagnostics: &[Diagnostic]) -> Result<String> {
    Ok(serde_json::to_string_pretty(diagnostics)?)
}

pub fn pretty(diagnostics: &[Diagnostic], cwd: &Path) -> String {
    if diagnostics.is_empty() {
        return format!(
            "{} No problems found!",
            "✓".if_supports_color(Stream::Stderr, |text| text.green())
        );
    }

    let mut output = String::new();
    for diagnostic in diagnostics {
        let marker = match diagnostic.severity {
            Severity::Error => format!(
                "{}",
                "×".if_supports_color(Stream::Stderr, |text| text.red())
            ),
            Severity::Warn => format!(
                "{}",
                "▲".if_supports_color(Stream::Stderr, |text| text.yellow())
            ),
            Severity::Off => continue,
        };
        let path = diagnostic
            .location
            .path
            .strip_prefix(cwd)
            .unwrap_or(&diagnostic.location.path);
        let mut location = path.display().to_string();
        if let Some(line) = diagnostic.location.line {
            location.push_str(&format!(":{line}"));
            if let Some(column) = diagnostic.location.column {
                location.push_str(&format!(":{column}"));
            }
        }
        let rule = diagnostic
            .rule_name
            .if_supports_color(Stream::Stderr, |text| text.blue());
        let location = location.if_supports_color(Stream::Stderr, |text| text.underline());
        output.push_str(&format!(
            "┌ {}\n{marker} {}\n",
            location, diagnostic.message
        ));
        if !diagnostic.fixes.is_empty() {
            output.push_str(&format!(
                "{}\n│\n",
                "√ Auto-fixable".if_supports_color(Stream::Stderr, |text| text.green())
            ));
        } else {
            output.push_str("│\n");
        }
        output.push_str(&format!("└ {rule}: {}\n\n", diagnostic.description_url()));
    }

    let errors = diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.severity == Severity::Error)
        .count();
    let warnings = diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.severity == Severity::Warn)
        .count();
    let fixable = diagnostics
        .iter()
        .filter(|diagnostic| !diagnostic.fixes.is_empty())
        .count();
    let mut counts = Vec::new();
    if errors > 0 {
        counts.push(format!(
            "{}",
            format!("{errors} error{}", if errors == 1 { "" } else { "s" })
                .if_supports_color(Stream::Stderr, |text| text.red())
        ));
    }
    if warnings > 0 {
        counts.push(format!(
            "{}",
            format!("{warnings} warning{}", if warnings == 1 { "" } else { "s" })
                .if_supports_color(Stream::Stderr, |text| text.yellow())
        ));
    }
    let fixes = match fixable {
        0 => "none can be fixed automatically".to_owned(),
        count if count == diagnostics.len() => {
            "all can be fixed automatically with --fix".to_owned()
        }
        count => format!("{count} can be fixed automatically with --fix"),
    };
    output.push_str(&format!("Found {} ({fixes})", counts.join(" and ")));
    output
}
