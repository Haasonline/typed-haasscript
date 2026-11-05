package com.haasonline.typedhaasscript.configuration

import com.intellij.openapi.progress.EmptyProgressIndicator
import junit.framework.TestCase
import java.nio.file.Files

class ConfigurationInitializerTest : TestCase() {
    /** An existing `ths.toml` is reported without running `ths`. */
    fun testReportsAnExistingConfigurationWithoutRunningInit() {
        val directory = Files.createTempDirectory(TEMPORARY_PREFIX)
        try {
            val configuration = Files.createFile(ConfigurationDiscovery.configurationIn(directory))
            assertEquals(
                InitializationResult.AlreadyExists(configuration),
                ConfigurationInitializer.initialize(directory, EmptyProgressIndicator()),
            )
        } finally {
            directory.toFile().deleteRecursively()
        }
    }

    private companion object {
        const val TEMPORARY_PREFIX = "ths-init-test"
    }
}
