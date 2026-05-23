package com.rustyjavac.gradle

import org.gradle.api.provider.Property

abstract class RustyJavaCExtension {

    abstract val binaryPath: Property<String>

    abstract val javaVersion: Property<Int>
}
