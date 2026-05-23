package com.rustyjavac.gradle

import org.gradle.api.provider.ListProperty
import org.gradle.api.provider.Property

abstract class RustyJavaCExtension {

    abstract val command: ListProperty<String>

    abstract val javaVersion: Property<Int>
}
