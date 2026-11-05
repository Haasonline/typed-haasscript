package com.haasonline.typedhaasscript

import com.intellij.ide.plugins.PluginManagerCore
import com.intellij.openapi.extensions.PluginId
import com.intellij.testFramework.fixtures.BasePlatformTestCase
import org.jetbrains.plugins.textmate.TextMateService
import org.jetbrains.plugins.textmate.bundles.TextMateFileNameMatcher
import java.nio.file.Path

class HaasScriptBundleProviderTest : BasePlatformTestCase() {
    fun testTheBundleIsInTheInstalledPlugin() {
        assertEquals(1, HaasScriptBundleProvider.bundlesIn(installedPlugin()).size)
    }

    /** The IDE's own reader takes the VSCode extension's files as a bundle. */
    fun testEachGrammarIsReadForItsFileExtension() {
        val bundle = HaasScriptBundleProvider.bundlesIn(installedPlugin()).single()
        val reader = TextMateService.getInstance().readBundle(bundle.path)
        assertNotNull("the bundle cannot be read", reader)
        val extensionsByScope = reader!!.readGrammars().associate { grammar ->
            val scope = grammar.plist.value.getPlistValue(SCOPE_NAME_KEY)?.string
            scope to grammar.fileNameMatchers.filterIsInstance<TextMateFileNameMatcher.Extension>().map { it.extension }
        }
        assertEquals(listOf(TYPED_EXTENSION), extensionsByScope[TYPED_SCOPE])
        assertEquals(listOf(PLAIN_EXTENSION), extensionsByScope[PLAIN_SCOPE])
    }

    /**
     * The plugin as the test IDE installed it. Tests load its classes from
     * the build, not through the plugin, so it is found by its id.
     */
    private fun installedPlugin(): Path = PluginManagerCore.getPlugin(PluginId.getId(PLUGIN_ID))!!.pluginPath

    private companion object {
        /** The `<id>` of plugin.xml. */
        const val PLUGIN_ID = "com.haasonline.typedhaasscript"
        const val SCOPE_NAME_KEY = "scopeName"
        const val TYPED_SCOPE = "source.typed-haasscript"
        const val PLAIN_SCOPE = "source.haasscript"
        const val TYPED_EXTENSION = "ths"
        const val PLAIN_EXTENSION = "hs"
    }
}
