use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};
use ignore::WalkBuilder;

use crate::Config;

pub const LAYER_SEQUENCE: &[&str] = &["shared", "entities", "features", "widgets", "pages", "app"];
pub const UNSLICED_LAYERS: &[&str] = &["shared", "app"];
pub const CONVENTIONAL_SEGMENTS: &[&str] = &["ui", "api", "lib", "model", "config"];
pub const CROSS_REFERENCE_TOKEN: &str = "@x";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntryKind {
    File,
    Folder,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub path: PathBuf,
    pub kind: EntryKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Slice {
    pub layer_name: String,
    pub name: String,
    pub path: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Segment {
    pub layer_name: String,
    pub slice_name: Option<String>,
    pub name: String,
    pub entry: Entry,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceLocation {
    pub layer_name: String,
    pub layer_path: PathBuf,
    pub slice_name: Option<String>,
    pub slice_path: Option<PathBuf>,
    pub segment_name: Option<String>,
}

#[derive(Debug)]
pub struct Project {
    pub root: PathBuf,
    pub files: Vec<PathBuf>,
    children: HashMap<PathBuf, Vec<Entry>>,
}

impl Project {
    pub fn scan(root: &Path, config: &Config) -> Result<Self> {
        let root = normalize_canonical_path(
            root.canonicalize()
                .with_context(|| format!("cannot access lint root {}", root.display()))?,
        );
        let mut files = Vec::new();

        let mut builder = WalkBuilder::new(&root);
        builder
            .standard_filters(true)
            .hidden(false)
            .follow_links(false)
            .filter_entry(|entry| {
                entry.depth() == 0
                    || !entry.path().components().any(|component| {
                        matches!(
                            component.as_os_str().to_str(),
                            Some(".git" | "node_modules" | "target")
                        )
                    })
            });

        for result in builder.build() {
            let entry = result.with_context(|| format!("failed to scan {}", root.display()))?;
            if entry.file_type().is_some_and(|kind| kind.is_file()) {
                let path = entry.into_path();
                if !config.is_global_ignored(&path) {
                    files.push(path);
                }
            }
        }
        files.sort();

        Ok(Self::from_files(root, files))
    }

    pub(crate) fn with_files(&self, files: Vec<PathBuf>) -> Self {
        Self::from_files(self.root.clone(), files)
    }

    fn from_files(root: PathBuf, files: Vec<PathBuf>) -> Self {
        let mut directories = BTreeSet::from([root.clone()]);
        for file in &files {
            for ancestor in file.parent().into_iter().flat_map(Path::ancestors) {
                if !ancestor.starts_with(&root) {
                    break;
                }
                directories.insert(ancestor.to_owned());
                if ancestor == root {
                    break;
                }
            }
        }

        let mut children: HashMap<PathBuf, Vec<Entry>> = HashMap::new();
        for directory in directories.iter().filter(|path| *path != &root) {
            if let Some(parent) = directory.parent() {
                children.entry(parent.to_owned()).or_default().push(Entry {
                    path: directory.clone(),
                    kind: EntryKind::Folder,
                });
            }
        }
        for file in &files {
            if let Some(parent) = file.parent() {
                children.entry(parent.to_owned()).or_default().push(Entry {
                    path: file.clone(),
                    kind: EntryKind::File,
                });
            }
        }
        for entries in children.values_mut() {
            entries.sort_by(|left, right| left.path.cmp(&right.path));
        }

        Self {
            root,
            files,
            children,
        }
    }

    pub fn children(&self, path: &Path) -> &[Entry] {
        self.children.get(path).map_or(&[], Vec::as_slice)
    }

    pub fn child_directories(&self, path: &Path) -> impl Iterator<Item = &PathBuf> {
        self.children(path)
            .iter()
            .filter(|entry| entry.kind == EntryKind::Folder)
            .map(|entry| &entry.path)
    }

    pub fn layers(&self) -> BTreeMap<String, PathBuf> {
        let mut layers: BTreeMap<String, PathBuf> = BTreeMap::new();
        for path in self.child_directories(&self.root) {
            let Some(actual_name) = file_name(path) else {
                continue;
            };
            let canonical = remove_layer_prefix(actual_name);
            if !LAYER_SEQUENCE.contains(&canonical) {
                continue;
            }

            match layers.get(canonical) {
                None => {
                    layers.insert(canonical.to_owned(), path.clone());
                }
                Some(existing)
                    if has_layer_prefix(actual_name)
                        && !has_layer_prefix(file_name(existing).unwrap_or("")) =>
                {
                    layers.insert(canonical.to_owned(), path.clone());
                }
                Some(_) => {}
            }
        }
        layers
    }

    pub fn is_sliced(layer_name: &str) -> bool {
        !UNSLICED_LAYERS.contains(&layer_name)
    }

    pub fn is_slice(&self, folder: &Path) -> bool {
        self.children(folder).iter().any(|child| {
            entry_name(child).is_some_and(|name| CONVENTIONAL_SEGMENTS.contains(&name))
        })
    }

    pub fn slices(&self, layer_name: &str, layer_path: &Path) -> BTreeMap<String, PathBuf> {
        let mut slices = BTreeMap::new();
        for child in self.child_directories(layer_path) {
            self.collect_slices(layer_name, layer_path, child, &mut slices);
        }
        slices
    }

    pub fn all_slices(&self) -> Vec<Slice> {
        self.layers()
            .into_iter()
            .filter(|(name, _)| Self::is_sliced(name))
            .flat_map(|(layer_name, layer_path)| {
                self.slices(&layer_name, &layer_path)
                    .into_iter()
                    .map(move |(name, path)| Slice {
                        layer_name: layer_name.clone(),
                        name,
                        path,
                    })
            })
            .collect()
    }

    pub fn segments(&self, parent: &Path) -> BTreeMap<String, Entry> {
        let mut segments = BTreeMap::new();
        for entry in self.children(parent) {
            if is_index(entry) {
                continue;
            }
            if let Some(name) = entry_name(entry) {
                segments.insert(name.to_owned(), entry.clone());
            }
        }
        segments
    }

    pub fn all_segments(&self) -> Vec<Segment> {
        let mut result = Vec::new();
        for (layer_name, layer_path) in self.layers() {
            if Self::is_sliced(&layer_name) {
                for (slice_name, slice_path) in self.slices(&layer_name, &layer_path) {
                    for (name, entry) in self.segments(&slice_path) {
                        result.push(Segment {
                            layer_name: layer_name.clone(),
                            slice_name: Some(slice_name.clone()),
                            name,
                            entry,
                        });
                    }
                }
            } else {
                for (name, entry) in self.segments(&layer_path) {
                    result.push(Segment {
                        layer_name: layer_name.clone(),
                        slice_name: None,
                        name,
                        entry,
                    });
                }
            }
        }
        result
    }

    pub fn indexes(&self, parent: &Path) -> Vec<PathBuf> {
        self.children(parent)
            .iter()
            .filter(|entry| is_index(entry))
            .map(|entry| entry.path.clone())
            .collect()
    }

    pub fn source_index(&self) -> BTreeMap<PathBuf, SourceLocation> {
        let mut index = BTreeMap::new();
        for (layer_name, layer_path) in self.layers() {
            for entry in self
                .children(&layer_path)
                .iter()
                .filter(|entry| entry.kind == EntryKind::File)
            {
                index.insert(
                    entry.path.clone(),
                    SourceLocation {
                        layer_name: layer_name.clone(),
                        layer_path: layer_path.clone(),
                        slice_name: None,
                        slice_path: None,
                        segment_name: None,
                    },
                );
            }

            if Self::is_sliced(&layer_name) {
                for (slice_name, slice_path) in self.slices(&layer_name, &layer_path) {
                    for (segment_name, segment) in self.segments(&slice_path) {
                        self.index_entry_files(
                            &segment,
                            SourceLocation {
                                layer_name: layer_name.clone(),
                                layer_path: layer_path.clone(),
                                slice_name: Some(slice_name.clone()),
                                slice_path: Some(slice_path.clone()),
                                segment_name: Some(segment_name),
                            },
                            &mut index,
                        );
                    }
                    for path in self.indexes(&slice_path) {
                        index.insert(
                            path,
                            SourceLocation {
                                layer_name: layer_name.clone(),
                                layer_path: layer_path.clone(),
                                slice_name: Some(slice_name.clone()),
                                slice_path: Some(slice_path.clone()),
                                segment_name: None,
                            },
                        );
                    }
                }
            } else {
                for (segment_name, segment) in self.segments(&layer_path) {
                    self.index_entry_files(
                        &segment,
                        SourceLocation {
                            layer_name: layer_name.clone(),
                            layer_path: layer_path.clone(),
                            slice_name: None,
                            slice_path: None,
                            segment_name: Some(segment_name),
                        },
                        &mut index,
                    );
                }
            }
        }
        index
    }

    pub fn descendant_directories(&self, root: &Path) -> Vec<PathBuf> {
        let mut descendants = Vec::new();
        let mut pending = vec![root.to_owned()];
        while let Some(directory) = pending.pop() {
            pending.extend(self.child_directories(&directory).cloned());
            descendants.push(directory);
        }
        descendants
    }

    fn collect_slices(
        &self,
        _layer_name: &str,
        layer_path: &Path,
        folder: &Path,
        slices: &mut BTreeMap<String, PathBuf>,
    ) {
        if self.is_slice(folder) {
            let name = relative_slash(layer_path, folder);
            slices.insert(name, folder.to_owned());
            return;
        }
        for child in self.child_directories(folder) {
            self.collect_slices(_layer_name, layer_path, child, slices);
        }
    }

    fn index_entry_files(
        &self,
        entry: &Entry,
        metadata: SourceLocation,
        index: &mut BTreeMap<PathBuf, SourceLocation>,
    ) {
        match entry.kind {
            EntryKind::File => {
                index.insert(entry.path.clone(), metadata);
            }
            EntryKind::Folder => {
                for child in self.children(&entry.path) {
                    self.index_entry_files(child, metadata.clone(), index);
                }
            }
        }
    }
}

#[cfg(target_os = "windows")]
fn normalize_canonical_path(path: PathBuf) -> PathBuf {
    let bytes = path.as_os_str().as_encoded_bytes();
    let normalized = if let Some(suffix) = bytes.strip_prefix(br"\\?\UNC\") {
        [br"\\".as_slice(), suffix].concat()
    } else if let Some(suffix) = bytes.strip_prefix(br"\\?\")
        && suffix.get(1) == Some(&b':')
    {
        suffix.to_vec()
    } else {
        return path;
    };

    // SAFETY: the prefix manipulation preserves the platform path encoding returned by
    // `OsStr::as_encoded_bytes` and only removes ASCII bytes from a canonical Windows path.
    unsafe { PathBuf::from(std::ffi::OsStr::from_encoded_bytes_unchecked(&normalized)) }
}

#[cfg(not(target_os = "windows"))]
fn normalize_canonical_path(path: PathBuf) -> PathBuf {
    path
}

pub fn is_index(entry: &Entry) -> bool {
    entry.kind == EntryKind::File
        && entry
            .path
            .file_stem()
            .and_then(|stem| stem.to_str())
            .and_then(|stem| stem.split('.').next())
            == Some("index")
}

pub fn file_name(path: &Path) -> Option<&str> {
    path.file_name().and_then(|name| name.to_str())
}

pub fn relative_slash(base: &Path, path: &Path) -> String {
    path.strip_prefix(base)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

fn entry_name(entry: &Entry) -> Option<&str> {
    match entry.kind {
        EntryKind::File => entry.path.file_stem().and_then(|name| name.to_str()),
        EntryKind::Folder => file_name(&entry.path),
    }
}

fn has_layer_prefix(name: &str) -> bool {
    name.starts_with('_')
        || (name.len() >= 2 && name.as_bytes()[0].is_ascii_digit() && name.as_bytes()[1] == b'_')
}

fn remove_layer_prefix(name: &str) -> &str {
    if let Some(stripped) = name.strip_prefix('_') {
        stripped
    } else if name.len() >= 2 && name.as_bytes()[0].is_ascii_digit() && name.as_bytes()[1] == b'_' {
        &name[2..]
    } else {
        name
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::tempdir;

    use super::*;

    #[test]
    fn recognizes_prefixed_layers_and_grouped_slices() {
        let temp = tempdir().unwrap();
        let root = temp.path().join("src");
        fs::create_dir_all(root.join("2_entities/catalog/product/ui")).unwrap();
        fs::write(root.join("2_entities/catalog/product/ui/Card.tsx"), "").unwrap();
        fs::create_dir_all(root.join("entities/ignored/ui")).unwrap();
        fs::write(root.join("entities/ignored/ui/Card.tsx"), "").unwrap();

        let project = Project::scan(&root, &Config::recommended()).unwrap();
        let layers = project.layers();
        assert_eq!(layers["entities"], project.root.join("2_entities"));
        assert!(
            project
                .slices("entities", &layers["entities"])
                .contains_key("catalog/product")
        );
    }

    #[test]
    fn indexes_neighboring_segment_folders_without_prefix_bleed() {
        let temp = tempdir().unwrap();
        let root = temp.path().join("src");
        let ui_file = root.join("entities/product/ui/Card.tsx");
        let ui_kit_file = root.join("entities/product/ui-kit/theme.ts");
        fs::create_dir_all(ui_file.parent().unwrap()).unwrap();
        fs::create_dir_all(ui_kit_file.parent().unwrap()).unwrap();
        fs::write(&ui_file, "").unwrap();
        fs::write(&ui_kit_file, "").unwrap();
        let ui_file = normalize_canonical_path(ui_file.canonicalize().unwrap());
        let ui_kit_file = normalize_canonical_path(ui_kit_file.canonicalize().unwrap());

        let project = Project::scan(&root, &Config::recommended()).unwrap();
        let index = project.source_index();

        assert_eq!(index[&ui_file].segment_name.as_deref(), Some("ui"));
        assert_eq!(index[&ui_kit_file].segment_name.as_deref(), Some("ui-kit"));
    }

    #[test]
    fn walks_only_the_requested_directory_subtree() {
        let temp = tempdir().unwrap();
        let root = temp.path().join("src");
        let ui = root.join("entities/product/ui");
        let nested = ui.join("buttons/icons");
        let neighbor = root.join("entities/product/ui-kit");
        fs::create_dir_all(&nested).unwrap();
        fs::create_dir_all(&neighbor).unwrap();
        fs::write(nested.join("Icon.tsx"), "").unwrap();
        fs::write(neighbor.join("theme.ts"), "").unwrap();

        let project = Project::scan(&root, &Config::recommended()).unwrap();
        let ui = normalize_canonical_path(ui.canonicalize().unwrap());
        let nested = normalize_canonical_path(nested.canonicalize().unwrap());
        let neighbor = normalize_canonical_path(neighbor.canonicalize().unwrap());
        let descendants = project.descendant_directories(&ui);

        assert!(descendants.contains(&ui));
        assert!(descendants.contains(&nested));
        assert!(!descendants.contains(&neighbor));
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn normalizes_windows_canonical_paths_for_resolver_keys() {
        assert_eq!(
            normalize_canonical_path(PathBuf::from(r"\\?\C:\repo\src")),
            PathBuf::from(r"C:\repo\src")
        );
        assert_eq!(
            normalize_canonical_path(PathBuf::from(r"\\?\UNC\server\share\src")),
            PathBuf::from(r"\\server\share\src")
        );
    }
}
