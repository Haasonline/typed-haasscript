package com.haasonline.typedhaasscript.configuration

import com.intellij.notification.NotificationGroupManager
import com.intellij.openapi.actionSystem.ActionManager
import com.intellij.testFramework.fixtures.BasePlatformTestCase

/** plugin.xml registers what the Kotlin side names by id. */
class ConfigurationRegistrationTest : BasePlatformTestCase() {
    fun testTheCreateActionIsRegistered() {
        assertInstanceOf(
            ActionManager.getInstance().getAction(CreateConfigurationAction.ACTION_ID),
            CreateConfigurationAction::class.java,
        )
    }

    fun testTheNotificationGroupIsRegistered() {
        assertNotNull(
            NotificationGroupManager.getInstance().getNotificationGroup(ConfigurationNotifications.NOTIFICATION_GROUP_ID),
        )
    }
}
