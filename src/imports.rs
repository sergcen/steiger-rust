use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};
use oxc_allocator::Allocator;
use oxc_ast::ast::{Argument, CallExpression, Expression, ImportDeclaration, ImportExpression};
use oxc_ast_visit::{Visit, walk};
use oxc_parser::Parser;
use oxc_resolver::{
    ResolveOptions, Resolver, TsconfigDiscovery, TsconfigOptions, TsconfigReferences,
};
use oxc_span::SourceType;
use rayon::prelude::*;

use crate::fsd::{Project, SourceLocation};

const SOURCE_EXTENSIONS: &[&str] = &[
    "js", "jsx", "ts", "tsx", "mjs", "cjs", "mts", "cts", "vue", "svelte", "astro",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportRecord {
    pub specifier: String,
    pub resolved: Option<PathBuf>,
}

#[derive(Debug, Default)]
pub struct ImportGraph {
    pub imports: BTreeMap<PathBuf, Vec<ImportRecord>>,
}

impl ImportGraph {
    pub fn build(
        project: &Project,
        source_index: &BTreeMap<PathBuf, SourceLocation>,
    ) -> Result<Self> {
        let source_paths = source_index
            .keys()
            .filter(|path| is_source_path(path))
            .cloned()
            .collect::<Vec<_>>();

        let extracted = source_paths
            .par_iter()
            .map(|path| extract_dependencies(path).map(|imports| (path.clone(), imports)))
            .collect::<Result<Vec<_>>>()?;

        let resolver = make_resolver(&project.root);
        let imports = extracted
            .into_par_iter()
            .map(|(importer, specifiers)| {
                let importer_directory = importer.parent().unwrap_or(&project.root);
                let records = specifiers
                    .into_iter()
                    .map(|specifier| {
                        let resolved = resolver
                            .resolve(importer_directory, &specifier)
                            .ok()
                            .map(|resolution| resolution.full_path().to_owned())
                            .filter(|path| is_source_path(path));
                        ImportRecord {
                            specifier,
                            resolved,
                        }
                    })
                    .collect();
                (importer, records)
            })
            .collect::<Vec<_>>()
            .into_iter()
            .collect();
        Ok(Self { imports })
    }
}

pub fn is_source_path(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| SOURCE_EXTENSIONS.contains(&extension))
}

fn make_resolver(root: &Path) -> Resolver {
    let tsconfig = find_up(root, "tsconfig.json").map(|config_file| {
        TsconfigDiscovery::Manual(TsconfigOptions {
            config_file,
            references: TsconfigReferences::Auto,
        })
    });
    let options = ResolveOptions {
        extensions: [
            ".tsx", ".ts", ".d.ts", ".jsx", ".js", ".mts", ".d.mts", ".cts", ".d.cts", ".mjs",
            ".cjs", ".vue", ".svelte", ".astro", ".json",
        ]
        .into_iter()
        .map(str::to_owned)
        .collect(),
        extension_alias: vec![
            (
                ".js".to_owned(),
                vec![".ts".to_owned(), ".tsx".to_owned(), ".js".to_owned()],
            ),
            (
                ".jsx".to_owned(),
                vec![".tsx".to_owned(), ".jsx".to_owned()],
            ),
            (
                ".mjs".to_owned(),
                vec![".mts".to_owned(), ".mjs".to_owned()],
            ),
            (
                ".cjs".to_owned(),
                vec![".cts".to_owned(), ".cjs".to_owned()],
            ),
        ],
        condition_names: vec!["node".to_owned(), "import".to_owned()],
        tsconfig,
        ..ResolveOptions::default()
    };
    Resolver::new(options)
}

fn find_up(start: &Path, file_name: &str) -> Option<PathBuf> {
    start
        .ancestors()
        .map(|directory| directory.join(file_name))
        .find(|candidate| candidate.is_file())
}

fn extract_dependencies(path: &Path) -> Result<Vec<String>> {
    let source = fs::read_to_string(path)
        .with_context(|| format!("cannot read source file {}", path.display()))?;
    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default();
    let fragments = match extension {
        "vue" | "svelte" => extract_script_tags(&source),
        "astro" => extract_astro_frontmatter(&source),
        _ => vec![source.as_str()],
    };

    let mut result = Vec::new();
    for fragment in fragments {
        let allocator = Allocator::default();
        let source_type = if matches!(extension, "vue" | "svelte" | "astro") {
            SourceType::from_path(Path::new("component.tsx"))
                .expect("tsx is a supported source type")
        } else {
            SourceType::from_path(path).unwrap_or_default()
        };
        let parsed = Parser::new(&allocator, fragment, source_type).parse();
        let mut visitor = DependencyVisitor::default();
        visitor.visit_program(&parsed.program);
        result.extend(visitor.dynamic_dependencies);
        result.extend(visitor.static_dependencies);
    }
    Ok(result)
}

#[derive(Default)]
struct DependencyVisitor {
    dynamic_dependencies: Vec<String>,
    static_dependencies: Vec<String>,
}

impl<'a> Visit<'a> for DependencyVisitor {
    fn visit_import_declaration(&mut self, declaration: &ImportDeclaration<'a>) {
        self.static_dependencies
            .push(declaration.source.value.to_string());
        walk::walk_import_declaration(self, declaration);
    }

    fn visit_import_expression(&mut self, expression: &ImportExpression<'a>) {
        if let Expression::StringLiteral(source) = &expression.source {
            self.dynamic_dependencies.push(source.value.to_string());
        }
        walk::walk_import_expression(self, expression);
    }

    fn visit_call_expression(&mut self, expression: &CallExpression<'a>) {
        if let Expression::Identifier(callee) = &expression.callee
            && callee.name == "require"
            && let Some(Argument::StringLiteral(source)) = expression.arguments.first()
        {
            self.static_dependencies.push(source.value.to_string());
        }
        walk::walk_call_expression(self, expression);
    }
}

fn extract_script_tags(source: &str) -> Vec<&str> {
    let mut fragments = Vec::new();
    let mut rest = source;
    while let Some(start) = rest.find("<script") {
        let after_start = &rest[start..];
        let Some(open_end) = after_start.find('>') else {
            break;
        };
        let body = &after_start[open_end + 1..];
        let Some(close_start) = body.find("</script>") else {
            break;
        };
        fragments.push(&body[..close_start]);
        rest = &body[close_start + "</script>".len()..];
    }
    fragments
}

fn extract_astro_frontmatter(source: &str) -> Vec<&str> {
    let trimmed = source.trim_start_matches(['\u{feff}', ' ', '\t', '\r', '\n']);
    let Some(body) = trimmed.strip_prefix("---") else {
        return Vec::new();
    };
    let body = body
        .strip_prefix("\r\n")
        .or_else(|| body.strip_prefix('\n'))
        .unwrap_or(body);
    body.find("\n---")
        .map_or_else(Vec::new, |end| vec![&body[..end]])
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::tempdir;

    use super::*;

    #[test]
    fn extracts_dependencies_in_upstream_query_order() {
        let temp = tempdir().unwrap();
        let path = temp.path().join("source.tsx");
        fs::write(
            &path,
            "import x from './x'; const y = require('./y'); import('./z'); export * from './not-an-import';",
        )
        .unwrap();
        assert_eq!(
            extract_dependencies(&path).unwrap(),
            vec!["./z".to_owned(), "./x".to_owned(), "./y".to_owned()]
        );
    }

    #[test]
    fn extracts_vue_script_dependencies() {
        let temp = tempdir().unwrap();
        let path = temp.path().join("Card.vue");
        fs::write(
            &path,
            "<template/><script setup lang=\"ts\">import x from './x'</script>",
        )
        .unwrap();
        assert_eq!(extract_dependencies(&path).unwrap(), vec!["./x".to_owned()]);
    }

    #[test]
    fn does_not_resolve_non_source_dependencies_into_the_fsd_graph() {
        let temp = tempdir().unwrap();
        let root = temp.path().join("src");
        let source = root.join("widgets/banner/ui/Banner.tsx");
        let asset = root.join("shared/assets/background.png");
        fs::create_dir_all(source.parent().unwrap()).unwrap();
        fs::create_dir_all(asset.parent().unwrap()).unwrap();
        fs::write(
            &source,
            "import background from '../../../shared/assets/background.png'",
        )
        .unwrap();
        fs::write(&asset, "not an actual png").unwrap();

        let project = Project::scan(&root, &crate::Config::recommended()).unwrap();
        let graph = ImportGraph::build(&project, &project.source_index()).unwrap();
        let source = source.canonicalize().unwrap();

        assert_eq!(graph.imports[&source].len(), 1);
        assert_eq!(graph.imports[&source][0].resolved, None);
    }
}
