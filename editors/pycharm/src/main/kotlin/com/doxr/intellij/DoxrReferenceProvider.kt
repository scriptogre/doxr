package com.doxr.intellij

import com.intellij.openapi.util.TextRange
import com.intellij.psi.PsiElement
import com.intellij.psi.PsiReference
import com.intellij.psi.PsiReferenceProvider
import com.intellij.util.ProcessingContext
import com.jetbrains.python.psi.PyStringLiteralExpression
import java.util.regex.Pattern

/**
 * Finds MkDocs and Sphinx cross-reference patterns in docstrings and creates
 * per-segment Python references for the dotted paths.
 */
class DoxrReferenceProvider : PsiReferenceProvider() {

    companion object {
        // [display text][dotted.path]
        private val MKDOCS_EXPLICIT = Pattern.compile("\\[[^\\]]*\\]\\[([a-zA-Z_][\\w.]*)\\]")

        // [dotted.path][]
        private val MKDOCS_AUTOREF = Pattern.compile("\\[([a-zA-Z_][\\w.]*)\\]\\[\\]")

        // :role:`~?dotted.path`
        private val SPHINX_XREF = Pattern.compile(
            ":(class|func|meth|mod|attr|exc|data|obj|const|type):`~?([^`]+)`"
        )
    }

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
        findRefs(MKDOCS_EXPLICIT, content, 1, contentOffset, stringLiteral, references)
        findRefs(MKDOCS_AUTOREF, content, 1, contentOffset, stringLiteral, references)

        // Extract Sphinx refs (group 2 = the dotted path).
        findRefs(SPHINX_XREF, content, 2, contentOffset, stringLiteral, references)

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
            // Must be a fully-qualified dotted path.
            if (!path.contains('.')) continue
            // Must start with lowercase (package name, not ClassName.method).
            if (!path[0].isLowerCase() && path[0] != '_') continue

            val pathStart = matcher.start(group) + (if (matcher.group(group).startsWith("~")) 1 else 0)
            createSegmentReferences(path, contentOffset + pathStart, element, out)
        }
    }

    /**
     * For a dotted path like `app.services.Foo`, create a separate reference for
     * each segment:
     *   - `app`      → resolves to the `app` package
     *   - `services` → resolves to `app.services`
     *   - `Foo`      → resolves to `app.services.Foo`
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
