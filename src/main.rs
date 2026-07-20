use std::{
    env,
    path::{Path, PathBuf},
    process::ExitCode,
    sync::mpsc,
    time::Duration,
};

use anyhow::{Context, Result, bail};
use clap::{Parser, ValueEnum};
use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use steiger::{
    Config, DiagnosticBaseline, LintOptions, Severity,
    engine::{apply_fixes, lint},
    reporter, rules,
};

#[derive(Debug, Parser)]
#[command(
    name = "steiger",
    version,
    about = "Fast Feature-Sliced Design architecture linter"
)]
struct Cli {
    /// Folder containing FSD layers. Defaults to ./src, ./app, or the current folder.
    path: Option<PathBuf>,

    /// Watch filesystem changes.
    #[arg(short, long)]
    watch: bool,

    /// Apply available auto-fixes.
    #[arg(long)]
    fix: bool,

    /// Exit with an error code if there are warnings.
    #[arg(long)]
    fail_on_warnings: bool,

    /// Output format.
    #[arg(long, value_enum, default_value_t = Reporter::Pretty)]
    reporter: Reporter,

    /// Explicit steiger.toml or steiger.json path.
    #[arg(long)]
    config: Option<PathBuf>,

    /// Checked-in JSON file with known diagnostics. Only new diagnostics fail the command.
    #[arg(long, visible_alias = "fsd-errors", value_name = "FILE")]
    baseline: Option<PathBuf>,

    /// Print every built-in rule and exit.
    #[arg(long)]
    list_rules: bool,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum Reporter {
    Pretty,
    Json,
}

fn main() -> ExitCode {
    match try_main() {
        Ok(code) => ExitCode::from(code),
        Err(error) => {
            eprintln!("steiger: {error:#}");
            ExitCode::from(2)
        }
    }
}

fn try_main() -> Result<u8> {
    let cli = Cli::parse();
    if cli.list_rules {
        for rule in rules::ALL_RULES {
            let default = if rules::RECOMMENDED_RULES.contains(rule) {
                "recommended"
            } else {
                "disabled"
            };
            println!("{rule}\t{default}");
        }
        return Ok(0);
    }

    let config = Config::discover(cli.config.as_deref())?;
    let root = choose_root(cli.path.as_deref())?;
    if cli.watch {
        watch(&root, &config, &cli)?;
        Ok(0)
    } else {
        run_once(&root, &config, &cli)
    }
}

fn choose_root(explicit: Option<&Path>) -> Result<PathBuf> {
    let path = if let Some(path) = explicit {
        path.to_owned()
    } else {
        let cwd = env::current_dir().context("cannot determine current directory")?;
        if cwd.join("src").is_dir() {
            cwd.join("src")
        } else if cwd.join("app").is_dir() {
            cwd.join("app")
        } else {
            cwd
        }
    };
    if !path.is_dir() {
        bail!("lint path is not a folder: {}", path.display());
    }
    path.canonicalize()
        .with_context(|| format!("cannot resolve lint path {}", path.display()))
}

fn run_once(root: &Path, config: &Config, cli: &Cli) -> Result<u8> {
    let result = lint(root, LintOptions { config })?;
    if cli.baseline.is_none() {
        print_report(&result.diagnostics, cli.reporter)?;
    }
    let remaining = if cli.fix {
        apply_fixes(&result.diagnostics)?
    } else {
        result.diagnostics
    };
    if let Some(path) = &cli.baseline {
        let baseline = DiagnosticBaseline::load(path)?;
        let new_diagnostics = baseline.retain_new(remaining);
        print_report(&new_diagnostics, cli.reporter)?;
        if !new_diagnostics.is_empty() {
            eprintln!(
                "steiger: {} new diagnostic{} not present in baseline {}",
                new_diagnostics.len(),
                if new_diagnostics.len() == 1 {
                    " is"
                } else {
                    "s are"
                },
                path.display()
            );
        }
        return Ok(u8::from(!new_diagnostics.is_empty()));
    }
    let has_errors = remaining
        .iter()
        .any(|diagnostic| diagnostic.severity == Severity::Error);
    let has_warnings = remaining
        .iter()
        .any(|diagnostic| diagnostic.severity == Severity::Warn);
    Ok(u8::from(
        has_errors || (cli.fail_on_warnings && has_warnings),
    ))
}

fn print_report(diagnostics: &[steiger::Diagnostic], format: Reporter) -> Result<()> {
    match format {
        Reporter::Pretty => eprintln!("{}", reporter::pretty(diagnostics, &env::current_dir()?)),
        Reporter::Json => println!("{}", reporter::json(diagnostics)?),
    }
    Ok(())
}

fn watch(root: &Path, config: &Config, cli: &Cli) -> Result<()> {
    let (sender, receiver) = mpsc::channel();
    let mut watcher: RecommendedWatcher = notify::recommended_watcher(move |event| {
        let _ = sender.send(event);
    })?;
    watcher.watch(root, RecursiveMode::Recursive)?;

    let _ = run_once(root, config, cli)?;
    loop {
        let event = receiver.recv().context("filesystem watcher stopped")??;
        if event.paths.iter().all(|path| is_ignored_watch_path(path)) {
            continue;
        }
        while receiver.recv_timeout(Duration::from_millis(500)).is_ok() {}
        if matches!(cli.reporter, Reporter::Pretty) {
            eprint!("\x1b[2J\x1b[H");
        }
        let _ = run_once(root, config, cli)?;
    }
}

fn is_ignored_watch_path(path: &Path) -> bool {
    path.components().any(|component| {
        matches!(
            component.as_os_str().to_str(),
            Some(".git" | "node_modules" | "target")
        )
    })
}
