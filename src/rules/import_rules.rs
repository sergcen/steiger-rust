use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Path, PathBuf},
};

use crate::{
    diagnostic::Diagnostic,
    fsd::{
        CROSS_REFERENCE_TOKEN, EntryKind, LAYER_SEQUENCE, Project, SourceLocation, UNSLICED_LAYERS,
    },
    imports::{ImportGraph, ImportRecord},
};

use super::*;

type ResolvedInternalImport<'a> = (
    &'a Path,
    &'a SourceLocation,
    &'a ImportRecord,
    &'a SourceLocation,
);

pub(crate) struct ImportAnalysis<'a> {
    graph: &'a ImportGraph,
    index: &'a BTreeMap<PathBuf, SourceLocation>,
    resolved: Vec<ResolvedInternalImport<'a>>,
}

impl<'a> ImportAnalysis<'a> {
    pub(crate) fn new(
        graph: &'a ImportGraph,
        index: &'a BTreeMap<PathBuf, SourceLocation>,
    ) -> Self {
        Self {
            graph,
            index,
            resolved: resolved_internal_imports(graph, index).collect(),
        }
    }
}

pub fn run_rule(rule: &str, project: &Project, analysis: &ImportAnalysis<'_>) -> Vec<Diagnostic> {
    match rule {
        FORBIDDEN_IMPORTS => forbidden_imports(&analysis.resolved),
        NO_CROSS_IMPORTS => no_cross_imports(&analysis.resolved),
        NO_HIGHER_LEVEL_IMPORTS => no_higher_level_imports(&analysis.resolved),
        IMPORT_LOCALITY => import_locality(&analysis.resolved),
        NO_PUBLIC_API_SIDESTEP => no_public_api_sidestep(project, &analysis.resolved),
        INSIGNIFICANT_SLICE => insignificant_slice(analysis.graph, analysis.index),
        _ => Vec::new(),
    }
}

fn forbidden_imports(imports: &[ResolvedInternalImport<'_>]) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    for &(source_path, source, record, target) in imports {
        if source.layer_name == target.layer_name && source.slice_name != target.slice_name {
            if source.slice_name.is_some()
                && target.slice_name.is_some()
                && !is_cross_import_public_api(record, source, target)
            {
                diagnostics.push(Diagnostic::new(
                    FORBIDDEN_IMPORTS,
                    format!(
                        "Forbidden cross-import from slice \"{}\".",
                        target.slice_name.as_deref().unwrap_or_default()
                    ),
                    source_path,
                ));
            }
        } else if is_higher_layer(source, target) {
            diagnostics.push(Diagnostic::new(
                FORBIDDEN_IMPORTS,
                format!(
                    "Forbidden import from higher layer \"{}\".",
                    target.layer_name
                ),
                source_path,
            ));
        }
    }
    diagnostics
}

fn no_cross_imports(imports: &[ResolvedInternalImport<'_>]) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    for &(source_path, source, record, target) in imports {
        if source.layer_name == target.layer_name
            && source.slice_name != target.slice_name
            && source.slice_name.is_some()
            && target.slice_name.is_some()
            && !is_cross_import_public_api(record, source, target)
        {
            diagnostics.push(Diagnostic::new(
                NO_CROSS_IMPORTS,
                format!(
                    "Forbidden cross-import from slice \"{}\".",
                    target.slice_name.as_deref().unwrap_or_default()
                ),
                source_path,
            ));
        }
    }
    diagnostics
}

fn no_higher_level_imports(imports: &[ResolvedInternalImport<'_>]) -> Vec<Diagnostic> {
    imports
        .iter()
        .copied()
        .filter(|(_, source, _, target)| is_higher_layer(source, target))
        .map(|(source_path, _, _, target)| {
            Diagnostic::new(
                NO_HIGHER_LEVEL_IMPORTS,
                format!(
                    "Forbidden import from higher layer \"{}\".",
                    target.layer_name
                ),
                source_path,
            )
        })
        .collect()
}

fn import_locality(imports: &[ResolvedInternalImport<'_>]) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    for &(source_path, source, record, target) in imports {
        let first_component = record.specifier.split('/').next().unwrap_or_default();
        let relative = matches!(first_component, "." | "..");
        let same_slice =
            source.layer_name == target.layer_name && source.slice_name == target.slice_name;
        if relative && !same_slice {
            diagnostics.push(Diagnostic::new(
                IMPORT_LOCALITY,
                format!(
                    "Import from \"{}\" should not be relative.",
                    record.specifier
                ),
                source_path,
            ));
        } else if !relative && same_slice {
            diagnostics.push(Diagnostic::new(
                IMPORT_LOCALITY,
                format!("Import from \"{}\" should be relative.", record.specifier),
                source_path,
            ));
        }
    }
    diagnostics
}

fn no_public_api_sidestep(
    project: &Project,
    imports: &[ResolvedInternalImport<'_>],
) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    for &(source_path, source, record, target) in imports {
        if source.layer_name == target.layer_name
            && (!Project::is_sliced(&target.layer_name) || source.slice_name == target.slice_name)
        {
            continue;
        }

        if Project::is_sliced(&target.layer_name) {
            if target
                .segment_name
                .as_deref()
                .is_some_and(|name| name != CROSS_REFERENCE_TOKEN)
            {
                diagnostics.push(sidestep_diagnostic(source_path, record));
            }
            continue;
        }

        let Some(segment_name) = target.segment_name.as_deref() else {
            continue;
        };
        let Some(segment) = project.segments(&target.layer_path).remove(segment_name) else {
            continue;
        };
        let Some(resolved) = record.resolved.as_deref() else {
            continue;
        };
        if segment.kind != EntryKind::Folder
            || project
                .indexes(&segment.path)
                .iter()
                .any(|index| index == resolved)
        {
            continue;
        }

        if target.layer_name == "shared" && matches!(segment_name, "ui" | "lib") {
            let Ok(path_in_segment) = resolved.strip_prefix(&segment.path) else {
                continue;
            };
            let Some(top_component) = path_in_segment.components().next() else {
                continue;
            };
            let top_level = segment.path.join(top_component);
            let is_top_level_folder = project
                .child_directories(&segment.path)
                .any(|candidate| candidate == &top_level);
            if is_top_level_folder
                && !project
                    .indexes(&top_level)
                    .iter()
                    .any(|index| index == resolved)
            {
                diagnostics.push(sidestep_diagnostic(source_path, record));
            }
        } else {
            diagnostics.push(sidestep_diagnostic(source_path, record));
        }
    }
    diagnostics
}

fn sidestep_diagnostic(source_path: &Path, record: &ImportRecord) -> Diagnostic {
    Diagnostic::new(
        NO_PUBLIC_API_SIDESTEP,
        format!(
            "Forbidden sidestep of public API when importing from \"{}\".",
            record.specifier
        ),
        source_path,
    )
}

fn insignificant_slice(
    graph: &ImportGraph,
    index: &BTreeMap<PathBuf, SourceLocation>,
) -> Vec<Diagnostic> {
    let mut references: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    let mut locations: BTreeMap<String, PathBuf> = BTreeMap::new();

    for (source_path, imports) in &graph.imports {
        let Some(source) = index.get(source_path) else {
            continue;
        };
        let source_key = location_key(source);
        locations
            .entry(source_key.clone())
            .or_insert_with(|| location_path(source));

        for record in imports {
            let Some(target) = record.resolved.as_ref().and_then(|path| index.get(path)) else {
                continue;
            };
            if source.layer_name == target.layer_name {
                continue;
            }
            let target_key = location_key(target);
            locations
                .entry(target_key.clone())
                .or_insert_with(|| location_path(target));
            if source_key != target_key {
                references
                    .entry(target_key)
                    .or_default()
                    .insert(source_key.clone());
            }
        }
        references.entry(source_key).or_default();
    }

    let mut diagnostics = Vec::new();
    for (source_key, target_keys) in references {
        let source_layer = source_key.split('/').next().unwrap_or_default();
        if !Project::is_sliced(source_layer) || source_layer == "pages" {
            continue;
        }
        let Some(location) = locations.get(&source_key) else {
            continue;
        };
        if target_keys.len() == 1 {
            let reference = target_keys.iter().next().expect("set length was checked");
            if UNSLICED_LAYERS.contains(&reference.as_str()) {
                if reference != "app" {
                    diagnostics.push(Diagnostic::new(
                        INSIGNIFICANT_SLICE,
                        format!(
                            "This slice has only one reference on layer \"{reference}\". Consider moving this code to \"{reference}\"."
                        ),
                        location,
                    ));
                }
            } else {
                diagnostics.push(Diagnostic::new(
                    INSIGNIFICANT_SLICE,
                    format!(
                        "This slice has only one reference in slice \"{reference}\". Consider merging them."
                    ),
                    location,
                ));
            }
        } else if target_keys.is_empty() {
            diagnostics.push(Diagnostic::new(
                INSIGNIFICANT_SLICE,
                "This slice has no references. Consider removing it.",
                location,
            ));
        }
    }
    diagnostics
}

fn resolved_internal_imports<'a>(
    graph: &'a ImportGraph,
    index: &'a BTreeMap<PathBuf, SourceLocation>,
) -> impl Iterator<
    Item = (
        &'a Path,
        &'a SourceLocation,
        &'a ImportRecord,
        &'a SourceLocation,
    ),
> {
    graph
        .imports
        .iter()
        .flat_map(move |(source_path, records)| {
            let source = index.get(source_path);
            records.iter().filter_map(move |record| {
                let source = source?;
                let target = record.resolved.as_ref().and_then(|path| index.get(path))?;
                Some((source_path.as_path(), source, record, target))
            })
        })
}

fn is_higher_layer(source: &SourceLocation, target: &SourceLocation) -> bool {
    let source_index = LAYER_SEQUENCE
        .iter()
        .position(|layer| *layer == source.layer_name);
    let target_index = LAYER_SEQUENCE
        .iter()
        .position(|layer| *layer == target.layer_name);
    matches!((source_index, target_index), (Some(source), Some(target)) if source < target)
}

fn is_cross_import_public_api(
    record: &ImportRecord,
    source: &SourceLocation,
    target: &SourceLocation,
) -> bool {
    let (Some(resolved), Some(source_slice), Some(target_slice_path)) = (
        record.resolved.as_deref(),
        source.slice_name.as_deref(),
        target.slice_path.as_deref(),
    ) else {
        return false;
    };
    let parent = resolved.parent().unwrap_or_else(|| Path::new(""));
    if is_index_path(resolved) {
        parent
            == target_slice_path
                .join(CROSS_REFERENCE_TOKEN)
                .join(source_slice)
    } else {
        resolved.file_stem().and_then(|name| name.to_str()) == Some(source_slice)
            && parent == target_slice_path.join(CROSS_REFERENCE_TOKEN)
    }
}

fn is_index_path(path: &Path) -> bool {
    path.file_stem()
        .and_then(|stem| stem.to_str())
        .and_then(|stem| stem.split('.').next())
        == Some("index")
}

fn location_key(location: &SourceLocation) -> String {
    location.slice_name.as_ref().map_or_else(
        || location.layer_name.clone(),
        |slice| format!("{}/{slice}", location.layer_name),
    )
}

fn location_path(location: &SourceLocation) -> PathBuf {
    location
        .slice_path
        .clone()
        .unwrap_or_else(|| location.layer_path.clone())
}
