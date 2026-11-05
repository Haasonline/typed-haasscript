package com.haasonline.typedhaasscript.configuration

import junit.framework.TestCase
import java.nio.file.Path

class ConfigurationPromptDecisionTest : TestCase() {
    fun testPromptsForAFileInTheProjectWithNoConfiguration() {
        assertTrue(decide())
    }

    fun testDoesNotPromptWhenAConfigurationExists() {
        assertFalse(decide(configurationExists = true))
    }

    fun testDoesNotPromptWhenDeclinedForTheProject() {
        assertFalse(decide(declinedForProject = true))
    }

    fun testDoesNotPromptTwiceInOneSession() {
        assertFalse(decide(alreadyPromptedThisSession = true))
    }

    fun testDoesNotPromptForAFileOutsideTheProject() {
        assertFalse(decide(filePath = Path.of("/", "downloads", "bot.ths")))
    }

    fun testDoesNotPromptForAFileInASiblingWithTheSamePrefix() {
        assertFalse(decide(filePath = Path.of("/", "projects", "trading-bot-old", "bot.ths")))
    }

    fun testDoesNotPromptWithoutAProjectDirectory() {
        assertFalse(decide(projectDirectory = null))
    }

    private fun decide(
        filePath: Path = PROJECT.resolve("scripts").resolve("bot.ths"),
        projectDirectory: Path? = PROJECT,
        configurationExists: Boolean = false,
        declinedForProject: Boolean = false,
        alreadyPromptedThisSession: Boolean = false,
    ): Boolean = ConfigurationPromptDecision.shouldPrompt(
        filePath = filePath,
        projectDirectory = projectDirectory,
        configurationExists = configurationExists,
        declinedForProject = declinedForProject,
        alreadyPromptedThisSession = alreadyPromptedThisSession,
    )

    private companion object {
        val PROJECT: Path = Path.of("/", "projects", "trading-bot")
    }
}
