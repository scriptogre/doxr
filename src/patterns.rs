/// Shared cross-reference patterns for docstring syntax detection.
///
/// SYNC: editors/pycharm/src/main/kotlin/com/doxr/intellij/DoxrPatterns.kt
/// These patterns MUST stay in sync with the Kotlin equivalents.
/// CI verifies this — see scripts/check_pattern_sync.sh
use regex::Regex;
use std::sync::LazyLock;

// ---------------------------------------------------------------------------
// MkDocs patterns
// ---------------------------------------------------------------------------

/// [display text][dotted.path]
pub static MKDOCS_EXPLICIT: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\[[^\]]*\]\[([a-zA-Z_][\w.]*)\]").unwrap());

/// [dotted.path][]  (autoref shorthand)
pub static MKDOCS_AUTOREF: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\[([a-zA-Z_][\w.]*)\]\[\]").unwrap());

// ---------------------------------------------------------------------------
// Sphinx patterns
// ---------------------------------------------------------------------------

/// :role:`~optional.dotted.path`
pub static SPHINX_XREF: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r":(class|func|meth|mod|attr|exc|data|obj|const|type):`~?([^`]+)`").unwrap()
});

// ---------------------------------------------------------------------------
// Rust-style patterns (intra-doc links)
// ---------------------------------------------------------------------------

/// [identifier] or [`identifier`]
pub static RUST_STYLE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\[`?([a-zA-Z_][\w.]*)`?\]").unwrap());

// ---------------------------------------------------------------------------
// Shared filtering logic
// ---------------------------------------------------------------------------

/// Check if a reference looks like a fully-qualified Python path.
///
/// Must contain a dot and start with a lowercase letter or underscore
/// (package names are lowercase; `ClassName.method` is a scoped ref, not FQN).
pub fn is_fully_qualified(s: &str) -> bool {
    s.contains('.') && s.starts_with(|c: char| c.is_ascii_lowercase() || c == '_')
}

/// Check if a Rust-style match should be skipped based on surrounding context.
///
/// Returns `true` if the match should be skipped (not a real cross-reference).
pub fn should_skip_rust_style(content: &[u8], match_start: usize, match_end: usize) -> bool {
    // Skip if preceded by:
    // - \ (escaped)
    // - ] (MkDocs [text][path] second part)
    // - word char (subscript like AbstractBase[int], not a cross-reference)
    if match_start > 0 {
        let prev = content[match_start - 1];
        if prev == b'\\' || prev == b']' || prev.is_ascii_alphanumeric() || prev == b'_' {
            return true;
        }
    }

    // Skip if followed by [ (MkDocs [path][] first part).
    if match_end < content.len() && content[match_end] == b'[' {
        return true;
    }

    false
}
