package com.doxr.intellij

import com.intellij.openapi.util.TextRange
import com.intellij.psi.PsiElement
import com.intellij.psi.PsiReference
import com.intellij.psi.PsiReferenceProvider
import com.intellij.util.ProcessingContext
import com.jetbrains.python.psi.PyStringLiteralExpression
import java.util.regex.Pattern

/**
 * Finds MkDocs, Sphinx, and Rust-style cross-reference patterns in docstrings
 * and creates per-segment Python references for the dotted paths.
 */
class DoxrReferenceProvider : PsiReferenceProvider() {

    override fun getReferencesByElement(
        element: PsiElement,
        context: ProcessingContext
    ): Array<PsiReference> {
        val stringLiteral = element as? PyStringLiteralExpression ?: return PsiReference.EMPTY_ARRAY

        // Only process triple-quoted strings (docstrings).
        val text = stringLiteral.text
        if (!text.startsWith("\"\"\"") && !text.startsWith("'''")) {
            return PsiReference.EMPTY_ARRAY
        }

        val references = mutableListOf<PsiReference>()
        val content = stringLiteral.stringValue
        // Offset from element start to the content start (past the opening quotes).
        val contentOffset = stringLiteral.stringValueTextRanges.firstOrNull()?.startOffset ?: return PsiReference.EMPTY_ARRAY

        // Extract MkDocs refs.
        findRefs(DoxrPatterns.MKDOCS_EXPLICIT, content, 1, contentOffset, stringLiteral, references)
        findRefs(DoxrPatterns.MKDOCS_AUTOREF, content, 1, contentOffset, stringLiteral, references)

        // Extract Sphinx refs (group 2 = the dotted path).
        findRefs(DoxrPatterns.SPHINX_XREF, content, 2, contentOffset, stringLiteral, references)

        // Collect offsets from MkDocs/Sphinx refs to avoid double-extracting.
        val existingOffsets = references.mapNotNull { ref ->
            (ref as? DoxrPythonReference)?.rangeInElement?.startOffset
        }.toSet()

        // Extract Rust-style refs.
        findRustStyleRefs(content, contentOffset, stringLiteral, existingOffsets, references)

        return references.toTypedArray()
    }

    private fun findRefs(
        pattern: Pattern,
        content: String,
        group: Int,
        contentOffset: Int,
        element: PyStringLiteralExpression,
        out: MutableList<PsiReference>,
    ) {
        val matcher = pattern.matcher(content)
        while (matcher.find()) {
            var path = matcher.group(group) ?: continue
            // Strip Sphinx tilde prefix.
            if (path.startsWith("~")) path = path.substring(1)
            if (!DoxrPatterns.isFullyQualified(path)) continue

            val pathStart = matcher.start(group) + (if (matcher.group(group).startsWith("~")) 1 else 0)
            createSegmentReferences(path, contentOffset + pathStart, element, out)
        }
    }

    private fun findRustStyleRefs(
        content: String,
        contentOffset: Int,
        element: PyStringLiteralExpression,
        existingOffsets: Set<Int>,
        out: MutableList<PsiReference>,
    ) {
        val matcher = DoxrPatterns.RUST_STYLE.matcher(content)
        while (matcher.find()) {
            val start = matcher.start()
            val end = matcher.end()

            if (DoxrPatterns.shouldSkipRustStyle(content, start, end)) continue

            val path = matcher.group(1) ?: continue
            val pathStart = matcher.start(1)

            // Skip if this offset was already captured by MkDocs/Sphinx patterns.
            if (existingOffsets.contains(contentOffset + pathStart)) continue

            if (path.contains('.')) {
                // Fully qualified — same per-segment references as MkDocs/Sphinx.
                if (!DoxrPatterns.isFullyQualified(path)) continue
                createSegmentReferences(path, contentOffset + pathStart, element, out)
            } else {
                // Short name — single reference, resolved via file scope.
                val range = TextRange(contentOffset + pathStart, contentOffset + pathStart + path.length)
                out.add(DoxrPythonReference(element, range, path, resolveShort = true))
            }
        }
    }

    /**
     * For a dotted path like `app.services.Foo`, create a separate reference for
     * each segment:
     *   - `app`      -> resolves to the `app` package
     *   - `services` -> resolves to `app.services`
     *   - `Foo`      -> resolves to `app.services.Foo`
     *
     * This gives per-segment Ctrl+Click and squiggles — identical to import statements.
     */
    private fun createSegmentReferences(
        dottedPath: String,
        offsetInElement: Int,
        element: PyStringLiteralExpression,
        out: MutableList<PsiReference>,
    ) {
        val segments = dottedPath.split('.')
        var pos = offsetInElement

        for (i in segments.indices) {
            val segment = segments[i]
            val qualifiedName = segments.subList(0, i + 1).joinToString(".")
            val range = TextRange(pos, pos + segment.length)

            out.add(DoxrPythonReference(element, range, qualifiedName))
            pos += segment.length + 1 // +1 for the dot
        }
    }
}
