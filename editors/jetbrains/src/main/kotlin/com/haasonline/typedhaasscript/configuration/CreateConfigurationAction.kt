package com.haasonline.typedhaasscript.configuration

import com.intellij.openapi.actionSystem.ActionUpdateThread
import com.intellij.openapi.actionSystem.AnAction
import com.intellij.openapi.actionSystem.AnActionEvent
import java.nio.file.Path

/**
 * Tools | Create ths.toml: runs `ths init` in the project's base directory,
 * whether or not the prompt was turned off. `ths init` itself, through
 * [ConfigurationInitializer], says when one is already there.
 */
class CreateConfigurationAction : AnAction() {
    override fun getActionUpdateThread(): ActionUpdateThread = ActionUpdateThread.BGT

    override fun update(event: AnActionEvent) {
        event.presentation.isEnabledAndVisible = event.project?.basePath != null
    }

    override fun actionPerformed(event: AnActionEvent) {
        val project = event.project ?: return
        val projectDirectory = project.basePath?.let(Path::of) ?: return
        ConfigurationCreationTask.start(project, projectDirectory)
    }

    companion object {
        /** The `<action>` id in plugin.xml. */
        const val ACTION_ID = "com.haasonline.typedhaasscript.CreateConfiguration"
    }
}
