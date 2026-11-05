package com.haasonline.typedhaasscript.configuration

import com.haasonline.typedhaasscript.ServerSettings
import com.intellij.execution.ExecutionException
import com.intellij.execution.configurations.GeneralCommandLine
import com.intellij.execution.process.CapturingProcessHandler
import com.intellij.openapi.progress.ProgressIndicator
import java.nio.file.Files
import java.nio.file.Path

/** What running `ths init` in a directory came to. */
sealed interface InitializationResult {
    data class Created(val configuration: Path) : InitializationResult

    data class AlreadyExists(val configuration: Path) : InitializationResult

    data class Failed(val message: String) : InitializationResult
}

/**
 * Runs `<ths> init <directory>`, which owns what a new `ths.toml` holds.
 * Blocks until it finishes, so call it off the UI thread.
 */
object ConfigurationInitializer {
    private const val INIT_COMMAND = "init"

    fun initialize(directory: Path, indicator: ProgressIndicator): InitializationResult {
        val configuration = ConfigurationDiscovery.configurationIn(directory)
        if (Files.exists(configuration)) return InitializationResult.AlreadyExists(configuration)

        val commandLine = GeneralCommandLine(ServerSettings.getInstance().serverPath, INIT_COMMAND, directory.toString())
            .withWorkDirectory(directory.toFile())
        val output = try {
            CapturingProcessHandler(commandLine).runProcessWithProgressIndicator(indicator)
        } catch (exception: ExecutionException) {
            return InitializationResult.Failed(exception.message ?: commandLine.commandLineString)
        }
        return if (output.exitCode == 0 && !output.isCancelled && !output.isTimeout) {
            InitializationResult.Created(configuration)
        } else {
            InitializationResult.Failed(output.stderr.ifBlank { output.stdout }.trim())
        }
    }
}
