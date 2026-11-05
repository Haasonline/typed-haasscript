package com.haasonline.typedhaasscript.configuration

import com.intellij.ide.util.PropertiesComponent
import com.intellij.openapi.project.Project

/** The owner's "Don't Ask Again" for the `ths.toml` prompt, kept per project. */
object ConfigurationPromptPreference {
    private const val DECLINED_PROPERTY = "com.haasonline.typedhaasscript.configurationPromptDeclined"

    fun isDeclined(project: Project): Boolean =
        PropertiesComponent.getInstance(project).getBoolean(DECLINED_PROPERTY)

    fun decline(project: Project) {
        PropertiesComponent.getInstance(project).setValue(DECLINED_PROPERTY, true)
    }
}
