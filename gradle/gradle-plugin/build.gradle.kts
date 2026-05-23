plugins {
    `java-gradle-plugin`
    kotlin("jvm") version "2.3.21"
    `maven-publish`
}

group = "com.rustyjavac"
version = "0.1.0"

repositories {
    mavenCentral()
}

dependencies {
    testImplementation(kotlin("test"))
}

gradlePlugin {
    plugins {
        create("rustyJavaC") {
            id = "com.rustyjavac"
            implementationClass = "com.rustyjavac.gradle.RustyJavaCPlugin"
            displayName = "Rusty-JavaC Compiler Plugin"
            description = "Gradle plugin that uses Rusty-JavaC (Rust-based Java compiler) to compile Java sources and produce JAR files"
            tags.set(listOf("java", "compiler", "rust", "rustyjavac"))
        }
    }
}

kotlin {
    jvmToolchain(21)
}
