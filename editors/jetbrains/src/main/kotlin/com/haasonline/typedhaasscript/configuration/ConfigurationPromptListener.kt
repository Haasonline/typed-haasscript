package com.haasonline.typedhaasscript.configuration

import com.haasonline.typedhaasscript.HaasScriptFiles
import com.intellij.openapi.application.ApplicationManager
import com.intellij.openapi.fileEditor.FileEditorManager
import com.intellij.openapi.fileEditor.FileEditorManagerListener
import com.intellij.openapi.project.Project
import com.intellij.openapi.vfs.VirtualFile
import java.nio.file.Files
import java.nio.file.Path

/**
 * Offers to create a `ths.toml` when a HaasScript file with none above it
 * is opened, since `ths server` checks only files that have one. The disk
 * is searched off the UI thread.
 */
class ConfigurationPromptListener : FileEditorManagerListener {
    override fun fileOpened(source: FileEditorManager, file: VirtualFile) {
        if (!HaasScriptFiles.isChecked(file)) return
        val project = source.project
        val session = ConfigurationPromptSession.getInstance(project)
        if (session.alreadyPrompted || ConfigurationPromptPreference.isDeclined(project)) return
        val filePath = file.toNioPath()
        ApplicationManager.getApplication().executeOnPooledThread {
            promptIfNeeded(project, session, filePath)
        }
    }

    private fun promptIfNeeded(project: Project, session: ConfigurationPromptSession, filePath: Path) {
        if (project.isDisposed) return
        val projectDirectory = project.basePath?.let(Path::of)
        val fileDirectory = filePath.parent ?: return
        val prompt = ConfigurationPromptDecision.shouldPrompt(
            filePath = filePath,
            projectDirectory = projectDirectory,
            configurationExists = ConfigurationDiscovery.nearestConfiguration(fileDirectory, Files::isRegularFile) != null,
            declinedForProject = ConfigurationPromptPreference.isDeclined(project),
            alreadyPromptedThisSession = session.alreadyPrompted,
        )
        if (prompt && projectDirectory != null && session.claimPrompt()) {
            ConfigurationNotifications.showPrompt(project, projectDirectory)
        }
    }
}
