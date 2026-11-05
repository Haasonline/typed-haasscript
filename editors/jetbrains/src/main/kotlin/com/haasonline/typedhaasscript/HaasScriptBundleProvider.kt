package com.haasonline.typedhaasscript

import com.intellij.openapi.application.PathManager
import org.jetbrains.plugins.textmate.api.TextMateBundleProvider
import org.jetbrains.plugins.textmate.api.TextMateBundleProvider.PluginBundle
import java.nio.file.Path
import kotlin.io.path.isDirectory

/**
 * Highlighting from the VSCode extension's TextMate grammars, which the
 * build copies into the plugin's `textmate` directory.
 */
class HaasScriptBundleProvider : TextMateBundleProvider {
    /**
     * The plugin is installed as `<plugin>/lib/<jar>`, so its directory is
     * two up from the jar holding this class. Asked of the jar because the
     * plugin-descriptor lookups are internal API.
     */
    override fun getBundles(): List<PluginBundle> =
        PathManager.getJarForClass(javaClass)?.parent?.parent?.let(::bundlesIn).orEmpty()

    companion object {
        /** Where build.gradle.kts puts the bundle. */
        private const val BUNDLE_DIRECTORY = "textmate"
        private const val BUNDLE_NAME = "Typed HaasScript"

        /** The bundle in the plugin installed at `pluginPath`. */
        internal fun bundlesIn(pluginPath: Path): List<PluginBundle> {
            val bundle = pluginPath.resolve(BUNDLE_DIRECTORY)
            return if (bundle.isDirectory()) listOf(PluginBundle(BUNDLE_NAME, bundle)) else emptyList()
        }
    }
}
