package com.haasonline.typedhaasscript.configuration

import com.intellij.openapi.components.Service
import com.intellij.openapi.project.Project
import java.util.concurrent.atomic.AtomicBoolean

/** Whether the `ths.toml` prompt has been shown in a project since the IDE started. */
@Service(Service.Level.PROJECT)
class ConfigurationPromptSession {
    private val prompted = AtomicBoolean(false)

    val alreadyPrompted: Boolean
        get() = prompted.get()

    /** Marks the prompt shown; false when another file opening got there first. */
    fun claimPrompt(): Boolean = prompted.compareAndSet(false, true)

    companion object {
        fun getInstance(project: Project): ConfigurationPromptSession =
            project.getService(ConfigurationPromptSession::class.java)
    }
}
