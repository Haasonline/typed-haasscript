package com.haasonline.typedhaasscript.configuration

import com.intellij.openapi.progress.ProgressIndicator
import com.intellij.openapi.progress.Task
import com.intellij.openapi.project.Project
import com.intellij.openapi.vfs.LocalFileSystem
import java.nio.file.Path

/**
 * Runs `ths init` for a directory in the background, then reports the
 * result. A new `ths.toml` is refreshed into the IDE's file system so the
 * language server's file watcher hears of it.
 */
class ConfigurationCreationTask private constructor(
    project: Project,
    private val directory: Path,
    private val onConfigurationPresent: () -> Unit,
) : Task.Backgroundable(project, "Creating ths.toml", true) {
    private var result: InitializationResult? = null

    override fun run(indicator: ProgressIndicator) {
        val initialization = ConfigurationInitializer.initialize(directory, indicator)
        if (initialization is InitializationResult.Created) {
            LocalFileSystem.getInstance().refreshAndFindFileByNioFile(initialization.configuration)
        }
        result = initialization
    }

    override fun onSuccess() {
        val initialization = result ?: return
        val project = project ?: return
        ConfigurationNotifications.showResult(project, initialization)
        if (initialization !is InitializationResult.Failed) onConfigurationPresent()
    }

    companion object {
        fun start(project: Project, directory: Path, onConfigurationPresent: () -> Unit = {}) {
            ConfigurationCreationTask(project, directory, onConfigurationPresent).queue()
        }
    }
}
