package com.rustyjavac.gradle

import org.gradle.api.Plugin
import org.gradle.api.Project
import org.gradle.api.tasks.bundling.Jar

class RustyJavaCPlugin : Plugin<Project> {

    override fun apply(project: Project) {
        val extension = project.extensions.create(
            "rustyJavaC",
            RustyJavaCExtension::class.java
        )

        extension.command.convention(listOf("rustyjavac"))
        extension.javaVersion.convention(21)

        project.plugins.withId("java") {
            configureForJavaPlugin(project, extension)
        }
    }

    private fun configureForJavaPlugin(
        project: Project,
        extension: RustyJavaCExtension
    ) {
        val javaExt = project.extensions.getByType(org.gradle.api.plugins.JavaPluginExtension::class.java)

        javaExt.sourceSets.all { sourceSet ->
            if (sourceSet.name == "test") {
                return@all
            }

            val compileTaskName = sourceSet.getTaskName("compile", "RustyJavaC")
            val jarTaskName = sourceSet.getTaskName("rustyJavaC", "Jar")
            val classesDir = project.layout.buildDirectory.dir("rustyjavac/${sourceSet.name}")

            val compileTask = project.tasks.register(compileTaskName, CompileRustyJavaCTask::class.java) {
                it.sourceFiles.setFrom(sourceSet.java.srcDirs)
                it.command.set(extension.command)
                it.javaVersion.set(extension.javaVersion)
                it.outputDir.set(classesDir)
                it.description = "Compiles ${sourceSet.name} Java sources using RustyJavaC"
                it.group = "rustyjavac"
            }

            project.tasks.register(jarTaskName, Jar::class.java) {
                it.from(classesDir)
                it.archiveBaseName.set(project.name)
                if (sourceSet.name != "main") {
                    it.archiveAppendix.set(sourceSet.name)
                }
                it.description = "Assembles a JAR from RustyJavaC-compiled ${sourceSet.name} classes"
                it.group = "rustyjavac"
                it.dependsOn(compileTask)
            }
        }
    }
}
