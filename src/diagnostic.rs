/// Diagnostics: validate docstring references and emit colored output.
use crate::config::DrefsConfig;
use crate::extract::{Reference, ReferenceKind, extract_references};
use crate::graph::SymbolGraph;
use crate::inventory::Inventory;
use colored::Colorize;
use std::path::Path;

/// Maximum edit distance for "did you mean?" suggestions.
const MAX_SUGGEST_DISTANCE: usize = 2;

/// A single diagnostic (error) to report.
#[derive(Debug)]
pub struct Diagnostic {
    pub file: String,
    pub line: usize,
    pub col: usize,
    pub code: &'static str,
    pub message: String,
    pub suggestion: Option<String>,
}

/// Format a diagnostic for terminal display with colors.
pub fn format_diagnostic(d: &Diagnostic, project_root: &Path) -> String {
    let rel_file = display_path(&d.file, project_root);
    let location = format!("{}:{}:{}", rel_file, d.line, d.col);
    let code = d.code.bold().bright_red();

    let message = colorize_message(&d.message);

    match &d.suggestion {
        Some(s) => {
            let help = format!("Did you mean `{}`?", s.bold());
            format!(
                "{}: {} {}\n  {} {}",
                location.bold(),
                code,
                message,
                "help:".bold().bright_cyan(),
                help
            )
        }
        None => format!("{}: {} {}", location.bold(), code, message),
    }
}

/// Colorize backtick-quoted symbols in a message string.
fn colorize_message(message: &str) -> String {
    let mut result = String::with_capacity(message.len());
    let mut in_backtick = false;
    let mut current = String::new();

    for ch in message.chars() {
        if ch == '`' {
            if in_backtick {
                // Closing backtick — emit the content as bold
                result.push('`');
                result.push_str(&current.bold().to_string());
                result.push('`');
                current.clear();
            }
            in_backtick = !in_backtick;
        } else if in_backtick {
            current.push(ch);
        } else {
            result.push(ch);
        }
    }

    // Handle unclosed backtick
    if in_backtick {
        result.push('`');
        result.push_str(&current);
    }

    result
}

/// Check all docstrings in the symbol graph and return diagnostics.
pub fn check(
    graph: &SymbolGraph,
    config: &DrefsConfig,
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
                                // Short name not in scope — suggest similar names.
                                let suggestion = graph.suggest_short_name(
                                    &r.target,
                                    module,
                                    MAX_SUGGEST_DISTANCE,
                                );
                                let message = if suggestion.is_some() {
                                    format!("Unresolved docstring reference `{}`", r.target)
                                } else {
                                    format!(
                                        "Unresolved docstring reference `{}`. No import or definition found in this file",
                                        r.target
                                    )
                                };
                                diagnostics.push(Diagnostic {
                                    file: file_path.clone(),
                                    line: docstring.line,
                                    col: docstring.col,
                                    code: "DREF001",
                                    message,
                                    suggestion,
                                });
                                continue;
                            }
                        }
                    }
                    ReferenceKind::FullyQualified => (r.target.clone(), false),
                };

                let display_name = if is_short { &r.target } else { &target };
                let internal = graph.is_internal(&target);

                if internal {
                    // Internal ref: must resolve in our symbol graph.
                    if !graph.resolve(&target) {
                        let suggestion = graph.suggest(&target, MAX_SUGGEST_DISTANCE);
                        diagnostics.push(Diagnostic {
                            file: file_path.clone(),
                            line: docstring.line,
                            col: docstring.col,
                            code: "DREF001",
                            message: format!("Unresolved docstring reference `{display_name}`"),
                            suggestion,
                        });
                    }
                } else if inventory.covers_root(&target) {
                    // External ref whose root module is covered by an inventory.
                    if !inventory.contains(&target) {
                        diagnostics.push(Diagnostic {
                            file: file_path.clone(),
                            line: docstring.line,
                            col: docstring.col,
                            code: "DREF001",
                            message: format!("Unresolved docstring reference `{display_name}`"),
                            suggestion: None,
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
fn is_explicitly_skipped(reference: &Reference, config: &DrefsConfig) -> bool {
    let root = reference.target.split('.').next().unwrap_or("");
    config.known_modules.iter().any(|km| {
        let km_root = km.split('.').next().unwrap_or("");
        root == km_root
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
        1 => format!("{}", "Found 1 error.".bold()),
        n => format!("{}", format!("Found {n} errors.").bold()),
    }
}

/// Relativize a file path for display.
pub fn display_path(path: &str, project_root: &Path) -> String {
    Path::new(path)
        .strip_prefix(project_root)
        .map(|p| p.display().to_string())
        .unwrap_or_else(|_| path.to_string())
}
