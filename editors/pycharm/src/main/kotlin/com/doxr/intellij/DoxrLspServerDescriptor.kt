package com.doxr.intellij

import com.intellij.execution.configurations.GeneralCommandLine
import com.intellij.openapi.project.Project
import com.intellij.openapi.vfs.VirtualFile
import com.intellij.platform.lsp.api.ProjectWideLspServerDescriptor
import java.io.File

class DoxrLspServerDescriptor(project: Project) :
    ProjectWideLspServerDescriptor(project, "doxr") {

    override fun isSupportedFile(file: VirtualFile): Boolean = file.extension == "py"

    override fun createCommandLine(): GeneralCommandLine {
        val doxr = findDoxrBinary()
        return GeneralCommandLine(doxr, "lsp")
            .withWorkDirectory(project.basePath)
            .withEnvironment("PATH", buildPath())
    }

    private fun findDoxrBinary(): String {
        // Check common install locations in order.
        val home = System.getProperty("user.home")
        val candidates = listOf(
            "$home/.local/bin/doxr",       // uv tool install
            "$home/.cargo/bin/doxr",       // cargo install
            "/usr/local/bin/doxr",
            "/opt/homebrew/bin/doxr",
        )
        for (path in candidates) {
            if (File(path).canExecute()) return path
        }
        // Fall back to PATH lookup.
        return "doxr"
    }

    private fun buildPath(): String {
        val home = System.getProperty("user.home")
        val extra = listOf(
            "$home/.local/bin",
            "$home/.cargo/bin",
            "/usr/local/bin",
            "/opt/homebrew/bin",
        )
        val existing = System.getenv("PATH") ?: ""
        return (extra + existing.split(":")).joinToString(":")
    }
}
