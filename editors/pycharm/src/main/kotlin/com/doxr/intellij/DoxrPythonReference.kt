package com.doxr.intellij

import com.intellij.openapi.util.TextRange
import com.intellij.psi.PsiElement
import com.intellij.psi.PsiFileSystemItem
import com.intellij.psi.PsiReferenceBase
import com.intellij.psi.util.QualifiedName
import com.jetbrains.python.psi.PyClass
import com.jetbrains.python.psi.PyFile
import com.jetbrains.python.psi.PyFunction
import com.jetbrains.python.psi.PyPsiFacade
import com.jetbrains.python.psi.PyStringLiteralExpression
import com.jetbrains.python.psi.PyUtil

/**
 * A reference from a docstring cross-reference segment to a Python symbol.
 *
 * Uses PyCharm's own resolution — the same code path as import statements.
 * Pattern taken from JetBrains' PyDocumentationLink.kt.
 */
class DoxrPythonReference(
    element: PyStringLiteralExpression,
    range: TextRange,
    private val qualifiedName: String,
) : PsiReferenceBase<PyStringLiteralExpression>(element, range, true) {

    override fun resolve(): PsiElement? {
        val project = element.project
        val facade = PyPsiFacade.getInstance(project)
        val qName = QualifiedName.fromDottedString(qualifiedName)
        val resolveContext = facade.createResolveContextFromFoothold(element)

        // Try as module/package first.
        val moduleResult = facade.resolveQualifiedName(qName, resolveContext)
            .asSequence()
            .filterIsInstance<PsiFileSystemItem>()
            .map { PyUtil.turnDirIntoInit(it) }
            .filterIsInstance<PyFile>()
            .firstOrNull()
        if (moduleResult != null) return moduleResult

        // Try as top-level function/class with member access.
        val withMembers = resolveContext.copyWithMembers()
        val directResult = facade.resolveQualifiedName(qName, withMembers).firstOrNull()
        if (directResult != null) return directResult

        // Try as a class method: resolve parent as class, find method by last segment.
        if (qName.componentCount > 1) {
            val parentQName = qName.removeLastComponent()
            val parentResult = facade.resolveQualifiedName(parentQName, withMembers)
                .filterIsInstance<PyClass>()
                .firstOrNull()
            if (parentResult != null) {
                val method = parentResult.findMethodByName(qName.lastComponent, true, null)
                if (method != null) return method
                // Try as class attribute.
                val attr = parentResult.findClassAttribute(qName.lastComponent.orEmpty(), true, null)
                if (attr != null) return attr
            }
        }

        return null
    }
}
