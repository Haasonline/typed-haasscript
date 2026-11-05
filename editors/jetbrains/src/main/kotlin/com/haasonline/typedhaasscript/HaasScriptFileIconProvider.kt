package com.haasonline.typedhaasscript

import com.intellij.ide.FileIconProvider
import com.intellij.openapi.project.Project
import com.intellij.openapi.util.IconLoader
import com.intellij.openapi.vfs.VirtualFile
import javax.swing.Icon

/**
 * The HaasScript icon for `.ths` and `.hs` files. Their file type stays the
 * TextMate one, which highlights them; only the icon changes. Dark themes
 * get `haasscript-file_dark.svg`, found by its name.
 */
class HaasScriptFileIconProvider : FileIconProvider {
    override fun getIcon(file: VirtualFile, flags: Int, project: Project?): Icon? =
        if (HaasScriptFiles.isHaasScript(file)) FILE_ICON else null

    private companion object {
        val FILE_ICON: Icon = IconLoader.getIcon("/icons/haasscript-file.svg", HaasScriptFileIconProvider::class.java)
    }
}
