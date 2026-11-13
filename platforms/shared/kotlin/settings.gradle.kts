// The Kotlin reference ports and the JVM contract tests (docs/ffi.md).
// Run: gradle -p platforms/shared/kotlin test   (after `cargo xtask bindings --lang kotlin`)

pluginManagement {
    repositories {
        gradlePluginPortal()
        mavenCentral()
    }
}

dependencyResolutionManagement {
    repositories {
        mavenCentral()
    }
}

rootProject.name = "jellyfesh-shared-kotlin"
