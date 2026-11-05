package com.haasonline.typedhaasscript

import com.intellij.openapi.options.BoundConfigurable
import com.intellij.openapi.project.ProjectManager
import com.intellij.openapi.ui.DialogPanel
import com.intellij.platform.lsp.api.LspClientManager
import com.intellij.ui.dsl.builder.AlignX
import com.intellij.ui.dsl.builder.bindText
import com.intellij.ui.dsl.builder.panel

/** Settings | Tools | Typed HaasScript. Applying a new path restarts the server. */
class ServerSettingsConfigurable : BoundConfigurable(DISPLAY_NAME) {
    private val settings = ServerSettings.getInstance()

    override fun createPanel(): DialogPanel = panel {
        row("Server path:") {
            textField()
                .bindText(settings::serverPath)
                .align(AlignX.FILL)
                .comment("The <code>ths</code> command: a path, or a name found on <code>PATH</code>.")
        }
    }

    override fun apply() {
        super.apply()
        for (project in ProjectManager.getInstance().openProjects) {
            LspClientManager.getInstance(project).stopAndRestartClientsIfNeeded(ThsIntegrationProvider::class.java)
        }
    }

    private companion object {
        const val DISPLAY_NAME = "Typed HaasScript"
    }
}
