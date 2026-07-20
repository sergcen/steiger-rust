use std::{
    collections::{BTreeMap, BTreeSet, HashSet},
    path::{Path, PathBuf},
    sync::OnceLock,
};

use pluralizer::pluralize;
use regex::Regex;
use strsim::levenshtein;

use crate::{
    diagnostic::{Diagnostic, Fix},
    fsd::{
        CONVENTIONAL_SEGMENTS, CROSS_REFERENCE_TOKEN, Entry, EntryKind, LAYER_SEQUENCE, Project,
        file_name,
    },
};

use super::*;

const BAD_SEGMENT_NAMES: &[&str] = &[
    "component",
    "components",
    "helper",
    "helpers",
    "util",
    "utils",
    "constant",
    "constants",
    "const",
    "consts",
    "type",
    "types",
    "store",
    "stores",
    "modal",
    "modals",
    "service",
    "services",
    "function",
    "functions",
    "class",
    "classes",
    "enum",
    "enums",
    "interface",
    "interfaces",
    "decorator",
    "decorators",
    "schema",
    "schemas",
    "handler",
    "handlers",
    "fixture",
    "fixtures",
    "middleware",
    "middlewares",
    "validator",
    "validators",
    "validation",
    "validations",
    "resolver",
    "resolvers",
    "mutation",
    "mutations",
    "asset",
    "assets",
    "hook",
    "hooks",
    "context",
    "provider",
    "providers",
    "composable",
    "composables",
    "directive",
    "directives",
    "action",
    "actions",
    "reducer",
    "reducers",
    "selector",
    "selectors",
    "effect",
    "effects",
    "saga",
    "sagas",
    "thunk",
    "thunks",
    "pipe",
    "pipes",
];

pub fn run_rule(rule: &str, project: &Project) -> Vec<Diagnostic> {
    match rule {
        AMBIGUOUS_SLICE_NAMES => ambiguous_slice_names(project),
        EXCESSIVE_SLICING => excessive_slicing(project),
        INCONSISTENT_NAMING => inconsistent_naming(project),
        NO_LAYER_PUBLIC_API => no_layer_public_api(project),
        NO_RESERVED_FOLDER_NAMES => no_reserved_folder_names(project),
        NO_SEGMENTLESS_SLICES => no_segmentless_slices(project),
        NO_SEGMENTS_ON_SLICED_LAYERS => no_segments_on_sliced_layers(project),
        NO_UI_IN_APP => no_ui_in_app(project),
        PUBLIC_API => public_api(project),
        REPETITIVE_NAMING => repetitive_naming(project),
        SEGMENTS_BY_PURPOSE => segments_by_purpose(project),
        SHARED_LIB_GROUPING => shared_lib_grouping(project),
        TYPO_IN_LAYER_NAME => typo_in_layer_name(project),
        NO_PROCESSES => no_processes(project),
        _ => Vec::new(),
    }
}

fn ambiguous_slice_names(project: &Project) -> Vec<Diagnostic> {
    let layers = project.layers();
    let Some(shared) = layers.get("shared") else {
        return Vec::new();
    };
    let shared_segments = project.segments(shared).into_keys().collect::<Vec<_>>();
    let mut diagnostics = Vec::new();

    for slice in project.all_slices() {
        let parts = slice.name.split('/').collect::<Vec<_>>();
        let Some((segment_index, matching)) = parts
            .iter()
            .enumerate()
            .find(|(_, part)| shared_segments.iter().any(|segment| segment == **part))
        else {
            continue;
        };

        if *matching == slice.name {
            diagnostics.push(Diagnostic::new(
                AMBIGUOUS_SLICE_NAMES,
                format!(
                    "Slice \"{}\" could be confused with a segment from Shared with the same name",
                    slice.name
                ),
                &slice.path,
            ));
        } else if segment_index == parts.len() - 1 {
            diagnostics.push(Diagnostic::new(
                AMBIGUOUS_SLICE_NAMES,
                format!(
                    "Slice \"{}\" could be confused with a segment \"{}\" from Shared",
                    slice.name, matching
                ),
                &slice.path,
            ));
        } else if let Some(layer_path) = layers.get(&slice.layer_name) {
            let group_name = parts[..=segment_index].join("/");
            let group_path = layer_path.join(&group_name);
            if project
                .child_directories(group_path.parent().unwrap_or(layer_path))
                .any(|path| path == &group_path)
            {
                diagnostics.push(Diagnostic::new(
                    AMBIGUOUS_SLICE_NAMES,
                    format!(
                        "Slice group \"{group_name}\" could be confused with a segment \"{matching}\" from Shared"
                    ),
                    group_path,
                ));
            }
        }
    }
    diagnostics
}

fn excessive_slicing(project: &Project) -> Vec<Diagnostic> {
    const THRESHOLD: usize = 20;
    let mut diagnostics = Vec::new();
    for (layer_name, layer_path) in project.layers() {
        if !matches!(
            layer_name.as_str(),
            "entities" | "features" | "widgets" | "pages"
        ) {
            continue;
        }
        for (group, slices) in group_slices(project.slices(&layer_name, &layer_path).into_keys()) {
            if slices.len() <= THRESHOLD {
                continue;
            }
            if group.is_empty() {
                diagnostics.push(Diagnostic::new(
                    EXCESSIVE_SLICING,
                    format!(
                        "Layer \"{layer_name}\" has {} ungrouped slices, which is above the recommended threshold of {THRESHOLD}. Consider grouping them or moving the code inside to the layer where it's used.",
                        slices.len()
                    ),
                    &layer_path,
                ));
            } else {
                diagnostics.push(Diagnostic::new(
                    EXCESSIVE_SLICING,
                    format!(
                        "Slice group \"{group}\" has {} slices, which is above the recommended threshold of {THRESHOLD}. Consider grouping them or moving the code inside to the layer where it's used.",
                        slices.len()
                    ),
                    layer_path.join(&group),
                ));
            }
        }
    }
    diagnostics
}

fn inconsistent_naming(project: &Project) -> Vec<Diagnostic> {
    let Some(entities) = project.layers().remove("entities") else {
        return Vec::new();
    };
    let neutral = ["k8s", "kubernetes", "media"];
    let mut diagnostics = Vec::new();
    for (group, names) in group_slices(project.slices("entities", &entities).into_keys()) {
        let names = names
            .into_iter()
            .filter(|name| !neutral.contains(&name.to_lowercase().as_str()))
            .collect::<Vec<_>>();
        let (plural, singular): (Vec<_>, Vec<_>) = names
            .into_iter()
            .partition(|name| pluralize(name, 2, false) == *name);
        if plural.is_empty() || singular.is_empty() {
            continue;
        }
        let location = if group.is_empty() {
            entities.clone()
        } else {
            entities.join(&group)
        };
        if plural.len() >= singular.len() {
            diagnostics.push(
                Diagnostic::new(
                    INCONSISTENT_NAMING,
                    "Inconsistent pluralization of slice names. Prefer all plural names",
                    &location,
                )
                .with_fixes(
                    singular
                        .into_iter()
                        .map(|name| Fix::Rename {
                            path: location.join(&name),
                            new_name: pluralize(&name, 2, false),
                        })
                        .collect(),
                ),
            );
        } else {
            diagnostics.push(
                Diagnostic::new(
                    INCONSISTENT_NAMING,
                    "Inconsistent pluralization of slice names. Prefer all singular names",
                    &location,
                )
                .with_fixes(
                    plural
                        .into_iter()
                        .map(|name| Fix::Rename {
                            path: location.join(&name),
                            new_name: pluralize(&name, 1, false),
                        })
                        .collect(),
                ),
            );
        }
    }
    diagnostics
}

fn no_layer_public_api(project: &Project) -> Vec<Diagnostic> {
    project
        .layers()
        .into_iter()
        .filter(|(layer_name, _)| layer_name != "app")
        .flat_map(|(layer_name, layer)| {
            project.indexes(&layer).into_iter().map(move |index| {
                Diagnostic::new(
                    NO_LAYER_PUBLIC_API,
                    format!("Layer \"{layer_name}\" should not have an index file"),
                    index,
                )
            })
        })
        .collect()
}

fn no_reserved_folder_names(project: &Project) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    for segment in project.all_segments() {
        if segment.entry.kind == EntryKind::File {
            continue;
        }
        for child in project.child_directories(&segment.entry.path) {
            for folder in project.descendant_directories(child) {
                let Some(name) = file_name(&folder) else {
                    continue;
                };
                if name == CROSS_REFERENCE_TOKEN {
                    diagnostics.push(Diagnostic::new(
                        NO_RESERVED_FOLDER_NAMES,
                        format!(
                            "Having a folder with the name \"{CROSS_REFERENCE_TOKEN}\" inside a segment could be confusing because that name is reserved for cross-import public APIs. Consider renaming it."
                        ),
                        folder,
                    ));
                } else if CONVENTIONAL_SEGMENTS.contains(&name) {
                    diagnostics.push(Diagnostic::new(
                        NO_RESERVED_FOLDER_NAMES,
                        format!(
                            "Having a folder with the name \"{name}\" inside a segment could be confusing because that name is commonly used for segments. Consider renaming it."
                        ),
                        folder,
                    ));
                }
            }
        }
    }
    diagnostics
}

fn no_segmentless_slices(project: &Project) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    for (layer_name, layer_path) in project.layers() {
        if !Project::is_sliced(&layer_name) {
            continue;
        }
        let mut candidates = project
            .child_directories(&layer_path)
            .cloned()
            .collect::<Vec<_>>();
        while let Some(candidate) = candidates.pop() {
            if is_slice_group(project, &candidate) {
                candidates.extend(project.child_directories(&candidate).cloned());
            } else if !project.is_slice(&candidate) {
                diagnostics.push(Diagnostic::new(
                    NO_SEGMENTLESS_SLICES,
                    "This slice has no segments. Consider dividing the code inside into segments.",
                    candidate,
                ));
            }
        }
    }
    diagnostics
}

fn is_slice_group(project: &Project, folder: &Path) -> bool {
    let children = project.children(folder);
    !children.is_empty()
        && !project.is_slice(folder)
        && children.iter().all(|child| {
            child.kind == EntryKind::Folder
                && (project.is_slice(&child.path)
                    || project
                        .children(&child.path)
                        .iter()
                        .all(|grandchild| grandchild.kind == EntryKind::File)
                    || is_slice_group(project, &child.path))
        })
}

fn no_segments_on_sliced_layers(project: &Project) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    for (layer_name, layer_path) in project.layers() {
        if !Project::is_sliced(&layer_name) {
            continue;
        }
        for child in project.child_directories(&layer_path) {
            let Some(name) = file_name(child) else {
                continue;
            };
            if CONVENTIONAL_SEGMENTS.contains(&name) {
                diagnostics.push(Diagnostic::new(
                    NO_SEGMENTS_ON_SLICED_LAYERS,
                    format!(
                        "Conventional segment \"{name}\" should not be a direct child of a sliced layer. Consider moving it inside a slice or, if that is a slice, consider a different name for it to avoid confusion with segments."
                    ),
                    child,
                ));
            }
        }
    }
    diagnostics
}

fn no_ui_in_app(project: &Project) -> Vec<Diagnostic> {
    let Some(app) = project.layers().remove("app") else {
        return Vec::new();
    };
    project
        .segments(&app)
        .get("ui")
        .map_or_else(Vec::new, |ui| {
            vec![Diagnostic::new(
                NO_UI_IN_APP,
                "Layer \"app\" should not have \"ui\" segment.",
                &ui.path,
            )]
        })
}

fn public_api(project: &Project) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    for (layer_name, layer_path) in project.layers() {
        if Project::is_sliced(&layer_name) {
            for slice_path in project.slices(&layer_name, &layer_path).into_values() {
                if project.indexes(&slice_path).is_empty() {
                    diagnostics.push(missing_public_api(
                        PUBLIC_API,
                        "This slice is missing a public API.",
                        slice_path,
                    ));
                }
            }
        } else if layer_name != "app" {
            for (segment_name, segment) in project.segments(&layer_path) {
                if !entry_indexes(project, &segment).is_empty() {
                    continue;
                }
                if !matches!(segment_name.as_str(), "ui" | "lib") {
                    diagnostics.push(missing_public_api(
                        PUBLIC_API,
                        "This segment is missing a public API.",
                        segment.path,
                    ));
                } else if segment.kind == EntryKind::Folder {
                    for child in project.child_directories(&segment.path) {
                        if project.indexes(child).is_empty() {
                            diagnostics.push(missing_public_api(
                                PUBLIC_API,
                                format!("This top-level folder in shared/{segment_name} is missing a public API."),
                                child,
                            ));
                        }
                    }
                }
            }
        }
    }
    diagnostics
}

fn missing_public_api(
    rule: &'static str,
    message: impl Into<String>,
    path: impl Into<PathBuf>,
) -> Diagnostic {
    let path = path.into();
    Diagnostic::new(rule, message, &path).with_fixes(vec![Fix::CreateFile {
        path: path.join("index.js"),
        content: String::new(),
    }])
}

fn entry_indexes(project: &Project, entry: &Entry) -> Vec<PathBuf> {
    match entry.kind {
        EntryKind::File => vec![entry.path.clone()],
        EntryKind::Folder => project.indexes(&entry.path),
    }
}

fn repetitive_naming(project: &Project) -> Vec<Diagnostic> {
    let word_pattern = word_pattern();
    let mut diagnostics = Vec::new();
    for (layer_name, layer_path) in project.layers() {
        if !Project::is_sliced(&layer_name) {
            continue;
        }
        for (group, names) in group_slices(project.slices(&layer_name, &layer_path).into_keys()) {
            let words = names
                .iter()
                .map(|name| {
                    word_pattern
                        .find_iter(name)
                        .map(|word| word.as_str().to_lowercase())
                        .collect::<HashSet<_>>()
                })
                .collect::<Vec<_>>();
            let mut counts = BTreeMap::new();
            for word in words.iter().flat_map(|set| set.iter()) {
                *counts.entry(word.clone()).or_insert(0_usize) += 1;
            }
            for (word, count) in counts {
                if names.len() > 2
                    && count >= names.len()
                    && words.iter().all(|set| set.contains(&word))
                {
                    diagnostics.push(Diagnostic::new(
                        REPETITIVE_NAMING,
                        format!("Repetitive word \"{word}\" in slice names."),
                        layer_path.join(&group),
                    ));
                }
            }
        }
    }
    diagnostics
}

fn word_pattern() -> &'static Regex {
    static PATTERN: OnceLock<Regex> = OnceLock::new();
    PATTERN.get_or_init(|| Regex::new(r"(?:[A-Z]+|[a-z]+)[a-z]*").unwrap())
}

fn segments_by_purpose(project: &Project) -> Vec<Diagnostic> {
    project
        .all_segments()
        .into_iter()
        .filter(|segment| BAD_SEGMENT_NAMES.contains(&segment.name.as_str()))
        .map(|segment| {
            Diagnostic::new(
                SEGMENTS_BY_PURPOSE,
                "This segment's name should describe the purpose of its contents, not what the contents are.",
                segment.entry.path,
            )
        })
        .collect()
}

fn shared_lib_grouping(project: &Project) -> Vec<Diagnostic> {
    const THRESHOLD: usize = 15;
    let Some(shared) = project.layers().remove("shared") else {
        return Vec::new();
    };
    let Some(lib) = project.segments(&shared).remove("lib") else {
        return Vec::new();
    };
    if lib.kind == EntryKind::Folder && project.children(&lib.path).len() > THRESHOLD {
        vec![Diagnostic::new(
            SHARED_LIB_GROUPING,
            format!(
                "Shared/lib has {} modules, which is above the recommended threshold of {THRESHOLD}. Consider grouping them.",
                project.children(&lib.path).len()
            ),
            lib.path,
        )]
    } else {
        Vec::new()
    }
}

fn typo_in_layer_name(project: &Project) -> Vec<Diagnostic> {
    let mut suggestions = project
        .child_directories(&project.root)
        .filter_map(|path| file_name(path).map(|name| (path, name)))
        .flat_map(|(path, input)| {
            LAYER_SEQUENCE.iter().filter_map(move |suggestion| {
                let distance = levenshtein(input, suggestion);
                (distance <= 3).then_some((path, input, *suggestion, distance))
            })
        })
        .collect::<Vec<_>>();
    suggestions.sort_by_key(|(_, _, _, distance)| *distance);

    let mut processed = BTreeSet::new();
    let mut claimed = BTreeSet::new();
    let mut diagnostics = Vec::new();
    for (path, input, suggestion, distance) in suggestions {
        if distance == 0 {
            claimed.insert(suggestion);
        } else if !processed.contains(input) && !claimed.contains(suggestion) {
            processed.insert(input);
            claimed.insert(suggestion);
            diagnostics.push(Diagnostic::new(
                TYPO_IN_LAYER_NAME,
                format!(
                    "Layer \"{input}\" potentially contains a typo. Did you mean \"{suggestion}\"?"
                ),
                path,
            ));
        }
    }
    diagnostics
}

fn no_processes(project: &Project) -> Vec<Diagnostic> {
    project
        .child_directories(&project.root)
        .find(|path| file_name(path) == Some("processes"))
        .map_or_else(Vec::new, |path| {
            vec![Diagnostic::new(
                NO_PROCESSES,
                "Layer \"processes\" is deprecated, avoid using it",
                path,
            )]
        })
}

fn group_slices(names: impl Iterator<Item = String>) -> BTreeMap<String, Vec<String>> {
    let mut groups: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for path in names {
        let (group, name) = path.rsplit_once('/').unwrap_or(("", &path));
        groups
            .entry(group.to_owned())
            .or_default()
            .push(name.to_owned());
    }
    groups
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::tempdir;

    use super::*;

    #[test]
    fn root_level_inconsistent_naming_location_has_no_trailing_separator() {
        let temp = tempdir().unwrap();
        let root = temp.path().join("src");
        fs::create_dir_all(root.join("entities/user/ui")).unwrap();
        fs::create_dir_all(root.join("entities/accounts/ui")).unwrap();
        fs::write(root.join("entities/user/ui/User.tsx"), "").unwrap();
        fs::write(root.join("entities/accounts/ui/Account.tsx"), "").unwrap();

        let project = Project::scan(&root, &crate::Config::recommended()).unwrap();
        let diagnostics = inconsistent_naming(&project);

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].location.path, project.root.join("entities"));
    }
}
