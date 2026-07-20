mod import_rules;
mod structural;

use crate::{diagnostic::Diagnostic, fsd::Project};

pub(crate) use import_rules::ImportAnalysis;

pub const AMBIGUOUS_SLICE_NAMES: &str = "fsd/ambiguous-slice-names";
pub const EXCESSIVE_SLICING: &str = "fsd/excessive-slicing";
pub const FORBIDDEN_IMPORTS: &str = "fsd/forbidden-imports";
pub const IMPORT_LOCALITY: &str = "fsd/import-locality";
pub const INCONSISTENT_NAMING: &str = "fsd/inconsistent-naming";
pub const INSIGNIFICANT_SLICE: &str = "fsd/insignificant-slice";
pub const NO_CROSS_IMPORTS: &str = "fsd/no-cross-imports";
pub const NO_HIGHER_LEVEL_IMPORTS: &str = "fsd/no-higher-level-imports";
pub const NO_LAYER_PUBLIC_API: &str = "fsd/no-layer-public-api";
pub const NO_PROCESSES: &str = "fsd/no-processes";
pub const NO_PUBLIC_API_SIDESTEP: &str = "fsd/no-public-api-sidestep";
pub const NO_RESERVED_FOLDER_NAMES: &str = "fsd/no-reserved-folder-names";
pub const NO_SEGMENTLESS_SLICES: &str = "fsd/no-segmentless-slices";
pub const NO_SEGMENTS_ON_SLICED_LAYERS: &str = "fsd/no-segments-on-sliced-layers";
pub const NO_UI_IN_APP: &str = "fsd/no-ui-in-app";
pub const PUBLIC_API: &str = "fsd/public-api";
pub const REPETITIVE_NAMING: &str = "fsd/repetitive-naming";
pub const SEGMENTS_BY_PURPOSE: &str = "fsd/segments-by-purpose";
pub const SHARED_LIB_GROUPING: &str = "fsd/shared-lib-grouping";
pub const TYPO_IN_LAYER_NAME: &str = "fsd/typo-in-layer-name";

pub const RECOMMENDED_RULES: &[&str] = &[
    AMBIGUOUS_SLICE_NAMES,
    EXCESSIVE_SLICING,
    FORBIDDEN_IMPORTS,
    INCONSISTENT_NAMING,
    INSIGNIFICANT_SLICE,
    NO_LAYER_PUBLIC_API,
    NO_PUBLIC_API_SIDESTEP,
    NO_RESERVED_FOLDER_NAMES,
    NO_SEGMENTLESS_SLICES,
    NO_SEGMENTS_ON_SLICED_LAYERS,
    NO_UI_IN_APP,
    PUBLIC_API,
    REPETITIVE_NAMING,
    SEGMENTS_BY_PURPOSE,
    SHARED_LIB_GROUPING,
    TYPO_IN_LAYER_NAME,
    NO_PROCESSES,
];

pub const ALL_RULES: &[&str] = &[
    AMBIGUOUS_SLICE_NAMES,
    EXCESSIVE_SLICING,
    FORBIDDEN_IMPORTS,
    INCONSISTENT_NAMING,
    INSIGNIFICANT_SLICE,
    NO_LAYER_PUBLIC_API,
    NO_PUBLIC_API_SIDESTEP,
    NO_RESERVED_FOLDER_NAMES,
    NO_SEGMENTLESS_SLICES,
    NO_SEGMENTS_ON_SLICED_LAYERS,
    NO_UI_IN_APP,
    PUBLIC_API,
    REPETITIVE_NAMING,
    SEGMENTS_BY_PURPOSE,
    SHARED_LIB_GROUPING,
    TYPO_IN_LAYER_NAME,
    NO_PROCESSES,
    NO_CROSS_IMPORTS,
    NO_HIGHER_LEVEL_IMPORTS,
    IMPORT_LOCALITY,
];

pub(crate) fn is_import_rule(rule: &str) -> bool {
    matches!(
        rule,
        FORBIDDEN_IMPORTS
            | IMPORT_LOCALITY
            | INSIGNIFICANT_SLICE
            | NO_CROSS_IMPORTS
            | NO_HIGHER_LEVEL_IMPORTS
            | NO_PUBLIC_API_SIDESTEP
    )
}

pub(crate) fn run_rule(
    rule: &str,
    project: &Project,
    analysis: &ImportAnalysis<'_>,
) -> Vec<Diagnostic> {
    if is_import_rule(rule) {
        import_rules::run_rule(rule, project, analysis)
    } else {
        structural::run_rule(rule, project)
    }
}
