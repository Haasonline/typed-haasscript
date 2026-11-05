package com.haasonline.typedhaasscript.configuration

import junit.framework.TestCase
import java.nio.file.Path

class ConfigurationDiscoveryTest : TestCase() {
    fun testFindsTheConfigurationInTheFilesOwnDirectory() {
        val configuration = SCRIPTS.resolve(ConfigurationDiscovery.CONFIGURATION_FILE_NAME)
        assertEquals(configuration, ConfigurationDiscovery.nearestConfiguration(SCRIPTS, setOf(configuration)::contains))
    }

    fun testFindsTheConfigurationInAnAncestorDirectory() {
        val configuration = PROJECT.resolve(ConfigurationDiscovery.CONFIGURATION_FILE_NAME)
        assertEquals(configuration, ConfigurationDiscovery.nearestConfiguration(SCRIPTS, setOf(configuration)::contains))
    }

    fun testPrefersTheNearestConfiguration() {
        val nearest = SCRIPTS.resolve(ConfigurationDiscovery.CONFIGURATION_FILE_NAME)
        val outer = PROJECT.resolve(ConfigurationDiscovery.CONFIGURATION_FILE_NAME)
        assertEquals(nearest, ConfigurationDiscovery.nearestConfiguration(SCRIPTS, setOf(nearest, outer)::contains))
    }

    fun testFindsTheConfigurationAtTheFilesystemRoot() {
        val configuration = ROOT.resolve(ConfigurationDiscovery.CONFIGURATION_FILE_NAME)
        assertEquals(configuration, ConfigurationDiscovery.nearestConfiguration(SCRIPTS, setOf(configuration)::contains))
    }

    fun testFindsNothingWhenNoDirectoryHasAConfiguration() {
        assertNull(ConfigurationDiscovery.nearestConfiguration(SCRIPTS) { false })
    }

    fun testIgnoresAConfigurationInASubdirectory() {
        val belowStart = SCRIPTS.resolve(ConfigurationDiscovery.CONFIGURATION_FILE_NAME)
        assertNull(ConfigurationDiscovery.nearestConfiguration(PROJECT, setOf(belowStart)::contains))
    }

    private companion object {
        val ROOT: Path = Path.of("/")
        val PROJECT: Path = ROOT.resolve("projects").resolve("trading-bot")
        val SCRIPTS: Path = PROJECT.resolve("scripts")
    }
}
