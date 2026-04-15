/// Diagnostics: validate references and emit Ruff-format output.
use crate::config::DoxrConfig;
use crate::extract::{extract_references, Reference, ReferenceKind};
use crate::graph::SymbolGraph;
use crate::inventory::Inventory;
use std::path::Path;

/// A single diagnostic (error) to report.
#[derive(Debug)]
pub struct Diagnostic {
    pub file: String,
    pub line: usize,
    pub col: usize,
    pub code: &'static str,
    pub message: String,
}

impl std::fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}:{}:{}: {} {}",
            self.file, self.line, self.col, self.code, self.message
        )
    }
}

/// Check all docstrings in the symbol graph and return diagnostics.
pub fn check(
    graph: &SymbolGraph,
    config: &DoxrConfig,
    inventory: &Inventory,
    file_map: &[(String, String)], // (dotted_path, file_path)
) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();

    for (dotted_path, file_path) in file_map {
        let module = match graph.modules.get(dotted_path) {
            Some(m) => m,
            None => continue,
        };

        for docstring in &module.docstrings {
            let refs = extract_references(&docstring.content, &config.style);
            for r in refs {
                if is_explicitly_skipped(&r, config) {
                    continue;
                }

                // Expand short names to fully-qualified paths.
                let (target, is_short) = match r.kind {
                    ReferenceKind::ShortName => {
                        match expand_short_name(&r.target, module) {
                            Some(fqn) => (fqn, true),
                            None => {
                                // Short name not in scope — error.
                                diagnostics.push(Diagnostic {
                                    file: file_path.clone(),
                                    line: docstring.line,
                                    col: docstring.col,
                                    code: "DXR001",
                                    message: format!("Unresolved reference `{}`", r.target),
                                });
                                continue;
                            }
                        }
                    }
                    ReferenceKind::FullyQualified => (r.target.clone(), false),
                };

                let internal = is_internal(&target, graph);

                if internal {
                    // Internal ref: must resolve in our symbol graph.
                    if !graph.resolve(&target) {
                        diagnostics.push(Diagnostic {
                            file: file_path.clone(),
                            line: docstring.line,
                            col: docstring.col,
                            code: "DXR001",
                            message: format!(
                                "Unresolved reference `{}`",
                                if is_short { &r.target } else { &target }
                            ),
                        });
                    }
                } else if inventory.covers_root(&target) {
                    // External ref whose root module is covered by an inventory:
                    // check it. If the root isn't covered, silently skip.
                    if !inventory.contains(&target) {
                        diagnostics.push(Diagnostic {
                            file: file_path.clone(),
                            line: docstring.line,
                            col: docstring.col,
                            code: "DXR001",
                            message: format!(
                                "Unresolved reference `{}`",
                                if is_short { &r.target } else { &target }
                            ),
                        });
                    }
                }
                // External ref not covered by any inventory: silently skip.
            }
        }
    }

    // Sort by file, then line, then column for stable output.
    diagnostics.sort_by(|a, b| {
        a.file
            .cmp(&b.file)
            .then(a.line.cmp(&b.line))
            .then(a.col.cmp(&b.col))
    });

    diagnostics
}

/// Check if a reference should be skipped entirely (explicit config).
fn is_explicitly_skipped(reference: &Reference, config: &DoxrConfig) -> bool {
    let root = reference.target.split('.').next().unwrap_or("");
    config.known_modules.iter().any(|km| {
        let km_root = km.split('.').next().unwrap_or("");
        root == km_root
    })
}

/// Check if a target's root module is part of the project we're checking.
fn is_internal(target: &str, graph: &SymbolGraph) -> bool {
    let root = target.split('.').next().unwrap_or("");
    graph.modules.keys().any(|path| {
        path == root || path.starts_with(&format!("{root}."))
    })
}

/// Expand a short name (e.g. `User`) to a fully-qualified path using the
/// module's imports and definitions. Returns `None` if the name isn't in scope.
fn expand_short_name(name: &str, module: &crate::graph::Module) -> Option<String> {
    // 1. Check imports.
    for imp in &module.imports {
        let local_name = imp.alias.as_deref().unwrap_or(&imp.name);
        if local_name == name {
            return Some(format!("{}.{}", imp.source, imp.name));
        }
    }

    // 2. Check local definitions.
    if module.definitions.contains_key(name) {
        return Some(format!("{}.{}", module.path, name));
    }

    None
}

/// Format diagnostics as a summary line.
pub fn summary(diagnostics: &[Diagnostic]) -> String {
    match diagnostics.len() {
        0 => "All references OK.".to_string(),
        1 => "Found 1 error.".to_string(),
        n => format!("Found {n} errors."),
    }
}

/// Relativize a file path for display.
pub fn display_path(path: &str, project_root: &Path) -> String {
    Path::new(path)
        .strip_prefix(project_root)
        .map(|p| p.display().to_string())
        .unwrap_or_else(|_| path.to_string())
}
