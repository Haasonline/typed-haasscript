package com.haasonline.typedhaasscript.configuration

import java.nio.file.Path

/** Whether opening a HaasScript file should offer to create a `ths.toml`. */
object ConfigurationPromptDecision {
    /**
     * True when the file at [filePath] has no `ths.toml` above it, sits in
     * [projectDirectory] (a lone file elsewhere has no obvious place for
     * one), and the owner has neither declined for good nor been asked
     * already this session.
     */
    fun shouldPrompt(
        filePath: Path,
        projectDirectory: Path?,
        configurationExists: Boolean,
        declinedForProject: Boolean,
        alreadyPromptedThisSession: Boolean,
    ): Boolean =
        !configurationExists &&
            !declinedForProject &&
            !alreadyPromptedThisSession &&
            projectDirectory != null &&
            isInside(filePath, projectDirectory)

    private fun isInside(filePath: Path, directory: Path): Boolean =
        filePath.toAbsolutePath().normalize().startsWith(directory.toAbsolutePath().normalize())
}
