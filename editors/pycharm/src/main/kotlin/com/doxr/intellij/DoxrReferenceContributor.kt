package com.doxr.intellij

import com.intellij.patterns.PlatformPatterns
import com.intellij.psi.*
import com.intellij.util.ProcessingContext
import com.jetbrains.python.psi.PyStringLiteralExpression

/**
 * Registers a reference provider on Python string literals (docstrings).
 * Detects cross-reference patterns and makes each dotted path segment
 * Ctrl+Clickable, with squiggles on unresolved segments.
 */
class DoxrReferenceContributor : PsiReferenceContributor() {
    override fun registerReferenceProviders(registrar: PsiReferenceRegistrar) {
        registrar.registerReferenceProvider(
            PlatformPatterns.psiElement(PyStringLiteralExpression::class.java),
            DoxrReferenceProvider()
        )
    }
}
