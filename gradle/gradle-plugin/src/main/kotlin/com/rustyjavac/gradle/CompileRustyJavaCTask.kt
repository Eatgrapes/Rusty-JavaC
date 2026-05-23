package com.rustyjavac.gradle

import org.gradle.api.DefaultTask
import org.gradle.api.GradleException
import org.gradle.api.file.ConfigurableFileCollection
import org.gradle.api.file.DirectoryProperty
import org.gradle.api.provider.ListProperty
import org.gradle.api.provider.Property
import org.gradle.api.tasks.Classpath
import org.gradle.api.tasks.Input
import org.gradle.api.tasks.OutputDirectory
import org.gradle.api.tasks.SkipWhenEmpty
import org.gradle.api.tasks.TaskAction
import org.gradle.work.DisableCachingByDefault

@DisableCachingByDefault(because = "RustyJavaC is an external process, caching not applicable")
abstract class CompileRustyJavaCTask : DefaultTask() {

    @get:Classpath
    @get:SkipWhenEmpty
    abstract val sourceFiles: ConfigurableFileCollection

    @get:Input
    abstract val command: ListProperty<String>

    @get:Input
    abstract val javaVersion: Property<Int>

    @get:OutputDirectory
    abstract val outputDir: DirectoryProperty

    @TaskAction
    fun compile() {
        val output = outputDir.get().asFile
        output.mkdirs()

        val sources = sourceFiles.files.flatMap { file ->
            if (file.isDirectory) {
                file.walkTopDown().filter { it.isFile && it.extension == "java" }.toList()
            } else if (file.extension == "java") {
                listOf(file)
            } else {
                emptyList()
            }
        }.map { it.absolutePath }
        if (sources.isEmpty()) {
            logger.lifecycle("No Java source files found, skipping RustyJavaC compilation")
            return
        }

        val cmd = mutableListOf<String>()
        cmd.addAll(command.get())
        cmd.add("--output-dir")
        cmd.add(output.absolutePath)
        cmd.addAll(sources)

        logger.lifecycle("RustyJavaC: compiling ${sources.size} source file(s)")
        logger.info("  command: ${cmd.joinToString(" ")}")

        val process = ProcessBuilder(cmd)
            .redirectErrorStream(true)
            .start()

        val stdout = process.inputStream.bufferedReader().readText()
        val exitCode = process.waitFor()

        if (stdout.isNotBlank()) {
            logger.lifecycle(stdout.trimEnd())
        }

        if (exitCode != 0) {
            throw GradleException("RustyJavaC compilation failed with exit code $exitCode")
        }

        val classFilesCount = output.walkTopDown().count { it.isFile && it.extension == "class" }
        logger.lifecycle("RustyJavaC: compiled $classFilesCount .class file(s) to ${output.absolutePath}")
    }
}
