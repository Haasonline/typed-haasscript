package com.haasonline.typedhaasscript

import com.intellij.openapi.vfs.VirtualFile

/**
 * The files `ths server` checks: Typed HaasScript (`.ths`, `.d.ths`) and
 * plain HaasScript (`.hs`). `.lua` files are left to the IDE's Lua support,
 * as in the VSCode extension.
 */
object HaasScriptFiles {
    private val CHECKED_EXTENSIONS = setOf("ths", "hs")

    fun isChecked(file: VirtualFile): Boolean = file.isInLocalFileSystem && isHaasScript(file)

    /** Whether the file is named as HaasScript, wherever it is stored. */
    fun isHaasScript(file: VirtualFile): Boolean = file.extension?.lowercase() in CHECKED_EXTENSIONS
}
