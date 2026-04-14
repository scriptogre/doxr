package com.doxr.intellij

import com.intellij.lang.annotation.AnnotationHolder
import com.intellij.lang.annotation.Annotator
import com.intellij.lang.annotation.HighlightSeverity
import com.intellij.openapi.editor.DefaultLanguageHighlighterColors
import com.intellij.openapi.util.TextRange
import com.intellij.psi.PsiElement
import com.jetbrains.python.psi.PyStringLiteralExpression
import java.util.regex.Pattern

/**
 * Highlights cross-reference dotted paths inside docstrings to look like
 * code identifiers rather than plain docstring text.
 */
class DoxrAnnotator : Annotator {

    companion object {
        private val MKDOCS_EXPLICIT = Pattern.compile("\\[[^\\]]*\\]\\[([a-zA-Z_][\\w.]*)\\]")
        private val MKDOCS_AUTOREF = Pattern.compile("\\[([a-zA-Z_][\\w.]*)\\]\\[\\]")
        private val SPHINX_XREF = Pattern.compile(
            ":(class|func|meth|mod|attr|exc|data|obj|const|type):`~?([^`]+)`"
        )
    }

    override fun annotate(element: PsiElement, holder: AnnotationHolder) {
        val stringLiteral = element as? PyStringLiteralExpression ?: return
        val text = stringLiteral.text
        if (!text.startsWith("\"\"\"") && !text.startsWith("'''")) return

        val content = stringLiteral.stringValue
        val contentOffset = stringLiteral.stringValueTextRanges.firstOrNull()?.startOffset ?: return
        val elementOffset = stringLiteral.textRange.startOffset

        highlightRefs(MKDOCS_EXPLICIT, content, 1, contentOffset, elementOffset, holder)
        highlightRefs(MKDOCS_AUTOREF, content, 1, contentOffset, elementOffset, holder)
        highlightRefs(SPHINX_XREF, content, 2, contentOffset, elementOffset, holder)
    }

    private fun highlightRefs(
        pattern: Pattern,
        content: String,
        group: Int,
        contentOffset: Int,
        elementOffset: Int,
        holder: AnnotationHolder,
    ) {
        val matcher = pattern.matcher(content)
        while (matcher.find()) {
            var path = matcher.group(group) ?: continue
            val tildeOffset = if (path.startsWith("~")) 1 else 0
            if (tildeOffset > 0) path = path.substring(1)
            if (!path.contains('.')) continue
            if (!path[0].isLowerCase() && path[0] != '_') continue

            val pathStart = matcher.start(group) + tildeOffset

            // Highlight each segment as an identifier.
            val segments = path.split('.')
            var pos = pathStart
            for (segment in segments) {
                val absStart = elementOffset + contentOffset + pos
                val absEnd = absStart + segment.length
                val range = TextRange(absStart, absEnd)

                holder.newSilentAnnotation(HighlightSeverity.INFORMATION)
                    .range(range)
                    .textAttributes(DefaultLanguageHighlighterColors.IDENTIFIER)
                    .create()

                pos += segment.length + 1 // +1 for dot
            }

            // Highlight dots as punctuation.
            pos = pathStart
            for (i in 0 until segments.size - 1) {
                pos += segments[i].length
                val dotStart = elementOffset + contentOffset + pos
                holder.newSilentAnnotation(HighlightSeverity.INFORMATION)
                    .range(TextRange(dotStart, dotStart + 1))
                    .textAttributes(DefaultLanguageHighlighterColors.DOT)
                    .create()
                pos += 1
            }
        }
    }
}
