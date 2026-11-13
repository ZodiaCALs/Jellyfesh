// Root build. Every Gradle plugin the modules use goes on this root classpath, and modules apply
// them by id without a version: one copy of each, and no "plugin already on the classpath with an
// unknown version" clash between a buildscript classpath and versioned plugins {} requests.
//
// The Android Gradle plugin joins only when settings found an Android SDK, so a JDK-only machine
// never has to download it (settings.gradle.kts). AGP 9 compiles Kotlin itself (built-in Kotlin)
// with the Kotlin Gradle plugin from this same classpath, pinned here to the catalog's Kotlin
// instead of AGP's own minimum.
buildscript {
    val withAndroid = gradle.extra.has("jellyfesh.android") && gradle.extra["jellyfesh.android"] == true
    repositories {
        if (withAndroid) {
            google {
                content {
                    includeGroupByRegex("com\\.android.*")
                    includeGroupByRegex("com\\.google\\.(android|testing|prefab).*")
                    includeGroupByRegex("androidx.*")
                }
            }
        }
        mavenCentral()
        gradlePluginPortal()
    }
    dependencies {
        classpath(libs.kotlin.gradle)
        classpath(libs.kotlin.compose.gradle)
        if (withAndroid) classpath(libs.android.gradle)
    }
}
