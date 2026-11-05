package com.haasonline.typedhaasscript.configuration

import com.intellij.notification.Notification
import com.intellij.notification.NotificationAction
import com.intellij.notification.NotificationGroupManager
import com.intellij.notification.NotificationType
import com.intellij.openapi.project.Project
import java.nio.file.Path

/** The balloons about a project's `ths.toml`. */
object ConfigurationNotifications {
    /** The `<notificationGroup>` id in plugin.xml. */
    internal const val NOTIFICATION_GROUP_ID = "Typed HaasScript"

    /** Offers to create a `ths.toml` in [projectDirectory]. */
    fun showPrompt(project: Project, projectDirectory: Path) {
        val folderName = projectDirectory.fileName?.toString() ?: projectDirectory.toString()
        val notification = notification(
            "Typed HaasScript checks scripts in folders with a ths.toml. Create one in $folderName?",
            NotificationType.INFORMATION,
        )
        notification.addAction(NotificationAction.createSimple("Create ths.toml") {
            ConfigurationCreationTask.start(project, projectDirectory) { notification.expire() }
        })
        notification.addAction(NotificationAction.createSimpleExpiring("Don't Ask Again") {
            ConfigurationPromptPreference.decline(project)
        })
        notification.notify(project)
    }

    /** Reports what `ths init` came to. */
    fun showResult(project: Project, result: InitializationResult) {
        val notification = when (result) {
            is InitializationResult.Created ->
                notification("Created ${result.configuration}.", NotificationType.INFORMATION)
            is InitializationResult.AlreadyExists ->
                notification("${result.configuration} already exists.", NotificationType.INFORMATION)
            is InitializationResult.Failed ->
                notification("Could not create ths.toml: ${result.message}", NotificationType.ERROR)
        }
        notification.notify(project)
    }

    private fun notification(content: String, type: NotificationType): Notification =
        NotificationGroupManager.getInstance()
            .getNotificationGroup(NOTIFICATION_GROUP_ID)
            .createNotification(content, type)
}
