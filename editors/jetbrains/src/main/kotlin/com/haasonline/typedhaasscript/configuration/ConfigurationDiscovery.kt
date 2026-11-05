package com.haasonline.typedhaasscript.configuration

import java.nio.file.Path

/**
 * Finds the `ths.toml` that `ths server` checks a file by: the one in the
 * file's directory or the nearest directory above it, as `tsc` finds
 * `tsconfig.json`.
 */
object ConfigurationDiscovery {
    const val CONFIGURATION_FILE_NAME = "ths.toml"

    /**
     * The nearest `ths.toml` in [directory] or above it, or null when there
     * is none up to the filesystem root. [isFile] says whether a path is a
     * file, so the walk itself touches no disk.
     */
    fun nearestConfiguration(directory: Path, isFile: (Path) -> Boolean): Path? =
        generateSequence(directory.toAbsolutePath().normalize()) { it.parent }
            .map { it.resolve(CONFIGURATION_FILE_NAME) }
            .firstOrNull(isFile)

    /** The `ths.toml` that `ths init` writes in [directory]. */
    fun configurationIn(directory: Path): Path = directory.resolve(CONFIGURATION_FILE_NAME)
}
