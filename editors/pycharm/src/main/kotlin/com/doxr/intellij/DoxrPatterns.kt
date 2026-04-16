package com.doxr.intellij

import java.util.regex.Pattern

/**
 * Shared cross-reference patterns for docstring syntax detection.
 *
 * SYNC: src/patterns.rs
 * These patterns MUST stay in sync with the Rust equivalents.
 * CI verifies this — see scripts/check_pattern_sync.sh
 */
object DoxrPatterns {

    // -----------------------------------------------------------------------
    // MkDocs patterns
    // -----------------------------------------------------------------------

    /** [display text][dotted.path] */
    val MKDOCS_EXPLICIT: Pattern = Pattern.compile("\\[[^\\]]*\\]\\[([a-zA-Z_][\\w.]*)\\]")

    /** [dotted.path][]  (autoref shorthand) */
    val MKDOCS_AUTOREF: Pattern = Pattern.compile("\\[([a-zA-Z_][\\w.]*)\\]\\[\\]")

    // -----------------------------------------------------------------------
    // Sphinx patterns
    // -----------------------------------------------------------------------

    /** :role:`~optional.dotted.path` */
    @Suppress("ktlint:standard:max-line-length")
    val SPHINX_XREF: Pattern = Pattern.compile(":(class|func|meth|mod|attr|exc|data|obj|const|type):`~?([^`]+)`")

    // -----------------------------------------------------------------------
    // Rust-style patterns (intra-doc links)
    // -----------------------------------------------------------------------

    /** [identifier] or [`identifier`] */
    val RUST_STYLE: Pattern = Pattern.compile("\\[`?([a-zA-Z_][\\w.]*)`?\\]")

    // -----------------------------------------------------------------------
    // Shared filtering logic
    // -----------------------------------------------------------------------

    /**
     * Check if a reference looks like a fully-qualified Python path.
     *
     * Must contain a dot and start with a lowercase letter or underscore
     * (package names are lowercase; `ClassName.method` is a scoped ref, not FQN).
     */
    fun isFullyQualified(path: String): Boolean {
        return path.contains('.') && (path[0].isLowerCase() || path[0] == '_')
    }

    /**
     * Check if a Rust-style match should be skipped based on surrounding context.
     *
     * Returns `true` if the match should be skipped (not a real cross-reference).
     */
    fun shouldSkipRustStyle(content: String, matchStart: Int, matchEnd: Int): Boolean {
        // Skip if preceded by:
        // - \ (escaped)
        // - ] (MkDocs [text][path] second part)
        // - word char (subscript like AbstractBase[int], not a cross-reference)
        if (matchStart > 0) {
            val prev = content[matchStart - 1]
            if (prev == '\\' || prev == ']' || prev.isLetterOrDigit() || prev == '_') {
                return true
            }
        }

        // Skip if followed by [ (MkDocs [path][] first part).
        if (matchEnd < content.length && content[matchEnd] == '[') {
            return true
        }

        return false
    }
}
