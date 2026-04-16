/// Extract cross-references from docstring content.
///
/// Supports MkDocs/mkdocstrings Markdown syntax, Sphinx RST syntax,
/// and Rust-style intra-doc links.
use crate::config::DocStyle;
use crate::patterns::{
    MKDOCS_AUTOREF, MKDOCS_EXPLICIT, RUST_STYLE, SPHINX_XREF, is_fully_qualified,
    should_skip_rust_style,
};

/// Whether a reference is fully qualified or a short name needing expansion.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReferenceKind {
    /// A dotted path like `pkg.models.User` — resolve directly.
    FullyQualified,
    /// A short name like `User` — expand via imports/definitions first.
    ShortName,
}

/// A cross-reference found in a docstring.
#[derive(Debug, Clone)]
pub struct Reference {
    /// The dotted path being referenced (e.g. `my_pkg.foo.Bar`).
    pub target: String,
    /// Byte offset within the docstring where this reference starts.
    pub offset: usize,
    /// Whether this is a fully-qualified path or a short name.
    pub kind: ReferenceKind,
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Extract all cross-references from a docstring's content.
pub fn extract_references(content: &str, style: &DocStyle) -> Vec<Reference> {
    let mut refs = match style {
        DocStyle::Mkdocs => extract_mkdocs(content),
        DocStyle::Sphinx => extract_sphinx(content),
        DocStyle::Auto => {
            let mut r = extract_mkdocs(content);
            r.extend(extract_sphinx(content));
            r
        }
    };

    // Always extract Rust-style refs, deduplicating against existing offsets.
    let existing_offsets: Vec<usize> = refs.iter().map(|r| r.offset).collect();
    refs.extend(extract_rust_style(content, &existing_offsets));

    refs
}

fn extract_mkdocs(content: &str) -> Vec<Reference> {
    let mut refs = Vec::new();

    for cap in MKDOCS_EXPLICIT.captures_iter(content) {
        if let Some(m) = cap.get(1)
            && is_fully_qualified(m.as_str())
        {
            refs.push(Reference {
                target: m.as_str().to_string(),
                offset: m.start(),
                kind: ReferenceKind::FullyQualified,
            });
        }
    }

    for cap in MKDOCS_AUTOREF.captures_iter(content) {
        if let Some(m) = cap.get(1)
            && is_fully_qualified(m.as_str())
        {
            refs.push(Reference {
                target: m.as_str().to_string(),
                offset: m.start(),
                kind: ReferenceKind::FullyQualified,
            });
        }
    }

    refs
}

fn extract_sphinx(content: &str) -> Vec<Reference> {
    let mut refs = Vec::new();

    for cap in SPHINX_XREF.captures_iter(content) {
        if let Some(m) = cap.get(2) {
            let target = m.as_str().trim();
            // Strip leading `~` if present (display hint, not part of path).
            let target = target.strip_prefix('~').unwrap_or(target);
            if is_fully_qualified(target) {
                refs.push(Reference {
                    target: target.to_string(),
                    offset: m.start(),
                    kind: ReferenceKind::FullyQualified,
                });
            }
        }
    }

    refs
}

fn extract_rust_style(content: &str, existing_offsets: &[usize]) -> Vec<Reference> {
    let mut refs = Vec::new();
    let bytes = content.as_bytes();

    for cap in RUST_STYLE.captures_iter(content) {
        let full_match = cap.get(0).unwrap();
        let start = full_match.start();
        let end = full_match.end();

        if should_skip_rust_style(bytes, start, end) {
            continue;
        }

        if let Some(m) = cap.get(1) {
            let target = m.as_str().to_string();

            // Skip if this offset was already captured by MkDocs/Sphinx patterns.
            if existing_offsets.contains(&m.start()) {
                continue;
            }

            let kind = if target.contains('.') {
                ReferenceKind::FullyQualified
            } else {
                ReferenceKind::ShortName
            };

            refs.push(Reference {
                target,
                offset: m.start(),
                kind,
            });
        }
    }

    refs
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mkdocs_explicit() {
        let content = r#"See [MyClass][my_pkg.foo.MyClass] for details."#;
        let refs = extract_references(content, &DocStyle::Mkdocs);
        assert_eq!(refs.len(), 1);
        assert_eq!(refs[0].target, "my_pkg.foo.MyClass");
    }

    #[test]
    fn test_mkdocs_autoref() {
        let content = r#"See [my_pkg.foo.MyClass][] for details."#;
        let refs = extract_references(content, &DocStyle::Mkdocs);
        assert_eq!(refs.len(), 1);
        assert_eq!(refs[0].target, "my_pkg.foo.MyClass");
    }

    #[test]
    fn test_sphinx_class() {
        let content = r#"See :class:`my_pkg.foo.MyClass` for details."#;
        let refs = extract_references(content, &DocStyle::Sphinx);
        assert_eq!(refs.len(), 1);
        assert_eq!(refs[0].target, "my_pkg.foo.MyClass");
    }

    #[test]
    fn test_sphinx_tilde() {
        let content = r#"See :func:`~my_pkg.foo.bar` for details."#;
        let refs = extract_references(content, &DocStyle::Sphinx);
        assert_eq!(refs.len(), 1);
        assert_eq!(refs[0].target, "my_pkg.foo.bar");
    }

    #[test]
    fn test_auto_finds_both() {
        let content = r#"
        See [my_pkg.foo.A][] and :class:`my_pkg.bar.B`.
        "#;
        let refs = extract_references(content, &DocStyle::Auto);
        assert_eq!(refs.len(), 2);
    }

    #[test]
    fn test_ignores_non_dotted_mkdocs() {
        let content = r#"See [details][section-link] for more."#;
        let refs = extract_references(content, &DocStyle::Mkdocs);
        assert_eq!(refs.len(), 0);
    }

    // -----------------------------------------------------------------------
    // Rust-style syntax tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_rust_style_bare_brackets_fq() {
        let content = r#"See [pkg.models.User] for details."#;
        let refs = extract_references(content, &DocStyle::Auto);
        assert_eq!(refs.len(), 1);
        assert_eq!(refs[0].target, "pkg.models.User");
        assert_eq!(refs[0].kind, ReferenceKind::FullyQualified);
    }

    #[test]
    fn test_rust_style_backtick_brackets_fq() {
        let content = "See [`pkg.models.User`] for details.";
        let refs = extract_references(content, &DocStyle::Auto);
        assert_eq!(refs.len(), 1);
        assert_eq!(refs[0].target, "pkg.models.User");
        assert_eq!(refs[0].kind, ReferenceKind::FullyQualified);
    }

    #[test]
    fn test_rust_style_short_name() {
        let content = r#"See [User] for details."#;
        let refs = extract_references(content, &DocStyle::Auto);
        assert_eq!(refs.len(), 1);
        assert_eq!(refs[0].target, "User");
        assert_eq!(refs[0].kind, ReferenceKind::ShortName);
    }

    #[test]
    fn test_rust_style_short_name_backticks() {
        let content = "See [`User`] for details.";
        let refs = extract_references(content, &DocStyle::Auto);
        assert_eq!(refs.len(), 1);
        assert_eq!(refs[0].target, "User");
        assert_eq!(refs[0].kind, ReferenceKind::ShortName);
    }

    #[test]
    fn test_rust_style_escaped_ignored() {
        let content = r#"See \[User] for details."#;
        let refs = extract_references(content, &DocStyle::Auto);
        assert_eq!(refs.len(), 0);
    }

    #[test]
    fn test_rust_style_no_collision_with_mkdocs_explicit() {
        let content = r#"See [display text][pkg.models.User] for details."#;
        let refs = extract_references(content, &DocStyle::Mkdocs);
        assert_eq!(refs.len(), 1);
        assert_eq!(refs[0].target, "pkg.models.User");
        assert_eq!(refs[0].kind, ReferenceKind::FullyQualified);
    }

    #[test]
    fn test_rust_style_no_collision_with_mkdocs_autoref() {
        let content = r#"See [pkg.models.User][] for details."#;
        let refs = extract_references(content, &DocStyle::Mkdocs);
        assert_eq!(refs.len(), 1);
        assert_eq!(refs[0].target, "pkg.models.User");
        assert_eq!(refs[0].kind, ReferenceKind::FullyQualified);
    }

    #[test]
    fn test_rust_style_ignores_non_identifiers() {
        let content = r#"See [see above] and [1] and [some/path] for details."#;
        let refs = extract_references(content, &DocStyle::Auto);
        assert_eq!(refs.len(), 0);
    }

    #[test]
    fn test_rust_style_mixed_with_mkdocs_and_sphinx() {
        let content = r#"
    Native: [User]
    MkDocs: [text][pkg.models.Admin]
    Sphinx: :class:`pkg.models.User`
    Native FQ: [pkg.sub.helper_func]
    "#;
        let refs = extract_references(content, &DocStyle::Auto);
        assert_eq!(refs.len(), 4);
    }

    #[test]
    fn test_rust_style_underscore_start() {
        let content = r#"See [_private_func] for details."#;
        let refs = extract_references(content, &DocStyle::Auto);
        assert_eq!(refs.len(), 1);
        assert_eq!(refs[0].target, "_private_func");
        assert_eq!(refs[0].kind, ReferenceKind::ShortName);
    }
}
