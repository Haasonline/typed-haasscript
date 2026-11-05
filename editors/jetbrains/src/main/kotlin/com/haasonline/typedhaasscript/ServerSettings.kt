package com.haasonline.typedhaasscript

import com.intellij.openapi.application.ApplicationManager
import com.intellij.openapi.components.PersistentStateComponent
import com.intellij.openapi.components.Service
import com.intellij.openapi.components.State
import com.intellij.openapi.components.Storage

/** Where `ths` is: a path, or a name found on `PATH`. Kept for every project. */
@Service(Service.Level.APP)
@State(name = "TypedHaasScriptServer", storages = [Storage("typed-haasscript.xml")])
class ServerSettings : PersistentStateComponent<ServerSettings.Saved> {
    class Saved {
        var serverPath: String = DEFAULT_SERVER_PATH
    }

    private var saved = Saved()

    var serverPath: String
        get() = saved.serverPath.ifBlank { DEFAULT_SERVER_PATH }
        set(value) {
            saved.serverPath = value.trim()
        }

    override fun getState(): Saved = saved

    override fun loadState(state: Saved) {
        saved = state
    }

    companion object {
        const val DEFAULT_SERVER_PATH = "ths"

        fun getInstance(): ServerSettings = ApplicationManager.getApplication().getService(ServerSettings::class.java)
    }
}
