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

    override fun annotate(element: PsiElement, holder: AnnotationHolder) {
        val stringLiteral = element as? PyStringLiteralExpression ?: return
        val text = stringLiteral.text
        if (!text.startsWith("\"\"\"") && !text.startsWith("'''")) return

        val content = stringLiteral.stringValue
        val contentOffset = stringLiteral.stringValueTextRanges.firstOrNull()?.startOffset ?: return
        val elementOffset = stringLiteral.textRange.startOffset

        // Track offsets from MkDocs/Sphinx patterns for dedup.
        val existingOffsets = mutableSetOf<Int>()

        collectOffsets(DoxrPatterns.MKDOCS_EXPLICIT, content, 1, existingOffsets)
        collectOffsets(DoxrPatterns.MKDOCS_AUTOREF, content, 1, existingOffsets)
        collectOffsets(DoxrPatterns.SPHINX_XREF, content, 2, existingOffsets)

        highlightRefs(DoxrPatterns.MKDOCS_EXPLICIT, content, 1, contentOffset, elementOffset, holder)
        highlightRefs(DoxrPatterns.MKDOCS_AUTOREF, content, 1, contentOffset, elementOffset, holder)
        highlightRefs(DoxrPatterns.SPHINX_XREF, content, 2, contentOffset, elementOffset, holder)

        highlightRustStyleRefs(content, contentOffset, elementOffset, existingOffsets, holder)
    }

    private fun collectOffsets(
        pattern: Pattern,
        content: String,
        group: Int,
        out: MutableSet<Int>,
    ) {
        val matcher = pattern.matcher(content)
        while (matcher.find()) {
            var path = matcher.group(group) ?: continue
            val tildeOffset = if (path.startsWith("~")) 1 else 0
            if (tildeOffset > 0) path = path.substring(1)
            if (!DoxrPatterns.isFullyQualified(path)) continue
            out.add(matcher.start(group) + tildeOffset)
        }
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
            if (!DoxrPatterns.isFullyQualified(path)) continue

            val pathStart = matcher.start(group) + tildeOffset
            highlightDottedPath(path, pathStart, contentOffset, elementOffset, holder)
        }
    }

    private fun highlightRustStyleRefs(
        content: String,
        contentOffset: Int,
        elementOffset: Int,
        existingOffsets: Set<Int>,
        holder: AnnotationHolder,
    ) {
        val matcher = DoxrPatterns.RUST_STYLE.matcher(content)
        while (matcher.find()) {
            val start = matcher.start()
            val end = matcher.end()

            if (DoxrPatterns.shouldSkipRustStyle(content, start, end)) continue

            val path = matcher.group(1) ?: continue
            val pathStart = matcher.start(1)

            if (existingOffsets.contains(pathStart)) continue

            if (path.contains('.')) {
                if (!DoxrPatterns.isFullyQualified(path)) continue
                highlightDottedPath(path, pathStart, contentOffset, elementOffset, holder)
            } else {
                // Short name — single identifier highlight.
                val absStart = elementOffset + contentOffset + pathStart
                val absEnd = absStart + path.length
                holder.newSilentAnnotation(HighlightSeverity.INFORMATION)
                    .range(TextRange(absStart, absEnd))
                    .textAttributes(DefaultLanguageHighlighterColors.IDENTIFIER)
                    .create()
            }
        }
    }

    private fun highlightDottedPath(
        path: String,
        pathStart: Int,
        contentOffset: Int,
        elementOffset: Int,
        holder: AnnotationHolder,
    ) {
        val segments = path.split('.')

        // Highlight each segment as an identifier.
        var pos = pathStart
        for (segment in segments) {
            val absStart = elementOffset + contentOffset + pos
            val absEnd = absStart + segment.length
            holder.newSilentAnnotation(HighlightSeverity.INFORMATION)
                .range(TextRange(absStart, absEnd))
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
