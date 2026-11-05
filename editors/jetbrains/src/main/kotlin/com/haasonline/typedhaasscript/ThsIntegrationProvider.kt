package com.haasonline.typedhaasscript

import com.intellij.execution.configurations.GeneralCommandLine
import com.intellij.openapi.project.Project
import com.intellij.openapi.vfs.VirtualFile
import com.intellij.platform.lsp.api.LspIntegrationProvider
import com.intellij.platform.lsp.api.ProjectWideLspClientDescriptor

/** Starts `ths server` once a HaasScript file is opened in the project. */
class ThsIntegrationProvider : LspIntegrationProvider {
    override fun fileOpened(
        project: Project,
        file: VirtualFile,
        clientStarter: LspIntegrationProvider.LspClientStarter,
    ) {
        if (HaasScriptFiles.isChecked(file)) {
            clientStarter.ensureClientStarted(ThsClientDescriptor(project))
        }
    }
}

/** One `ths server` for the whole project, started in its base directory. */
class ThsClientDescriptor(project: Project) : ProjectWideLspClientDescriptor(project, PRESENTABLE_NAME) {
    override fun isSupportedFile(file: VirtualFile): Boolean = HaasScriptFiles.isChecked(file)

    override fun createCommandLine(): GeneralCommandLine {
        val commandLine = GeneralCommandLine(ServerSettings.getInstance().serverPath, SERVER_COMMAND)
        project.basePath?.let { commandLine.withWorkDirectory(it) }
        return commandLine
    }

    private companion object {
        const val PRESENTABLE_NAME = "Typed HaasScript"
        const val SERVER_COMMAND = "server"
    }
}
