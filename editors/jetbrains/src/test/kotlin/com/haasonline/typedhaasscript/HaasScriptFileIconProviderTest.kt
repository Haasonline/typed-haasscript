package com.haasonline.typedhaasscript

import com.intellij.ide.FileIconProvider
import com.intellij.testFramework.fixtures.BasePlatformTestCase
import com.intellij.util.IconUtil

class HaasScriptFileIconProviderTest : BasePlatformTestCase() {
    fun testTheProviderIsRegistered() {
        assertTrue(FileIconProvider.EP_NAME.extensionList.any { it is HaasScriptFileIconProvider })
    }

    fun testTypedAndPlainScriptsGetTheHaasScriptIcon() {
        for (name in listOf("bot.ths", "library.d.ths", "bot.hs")) {
            val file = myFixture.tempDirFixture.createFile(name)
            assertNotNull(name, HaasScriptFileIconProvider().getIcon(file, 0, project))
        }
    }

    fun testOtherFilesKeepTheirIcon() {
        val file = myFixture.tempDirFixture.createFile("bot.lua")
        assertNull(HaasScriptFileIconProvider().getIcon(file, 0, project))
    }

    /** The IDE shows the provider's icon, though the file type is TextMate's. */
    fun testTheIdeShowsTheHaasScriptIcon() {
        val file = myFixture.tempDirFixture.createFile("bot.ths")
        val shown = IconUtil.getIcon(file, 0, project)
        assertEquals(HaasScriptFileIconProvider().getIcon(file, 0, project)!!.iconWidth, shown.iconWidth)
        assertTrue(shown.toString(), shown.toString().contains("haasscript-file"))
    }
}
