// Jellyfesh for Android.
//
//  :logic          pure Kotlin/JVM: state machines, formatting, drawing-time scheduling and
//                  settings models. Builds and tests anywhere with a JDK: `gradle :logic:test`.
//  :core-bindings  the UniFFI Kotlin bindings and tokens (generated, git-ignored), the shared
//                  drawing-time ports (platforms/shared/kotlin), JNA and the core's .so files
//                  built by cargo-ndk.
//  :app            the Compose app; its FFI adapter lives in app/src/ffi.
//  :ffi-check      JVM only, never shipped: compiles the app's FFI adapter with the generated
//                  bindings and the ports, and runs it against the host-built core. Present once
//                  `cargo xtask bindings --lang kotlin` has written the bindings.
//  :ui-check       JVM only, never shipped: compiles the whole app (but Media3) against Compose
//                  Multiplatform for desktop, Cloudy, android-all and stubs, for machines without
//                  an Android SDK. Present once the bindings and tokens are generated.
//
// The Android modules are only part of the build when an Android SDK is present (ANDROID_HOME,
// ANDROID_SDK_ROOT or local.properties `sdk.dir`), or with -Pjellyfesh.android=true. Without
// one, the Android Gradle plugin is never resolved, so :logic builds on a plain JDK.

pluginManagement {
    repositories {
        google {
            content {
                includeGroupByRegex("com\\.android.*")
                includeGroupByRegex("com\\.google\\.(android|testing|prefab).*")
                // Play Billing's data transport needs the Firebase encoders, which only Google's
                // Maven hosts (com.google.firebase:firebase-encoders, -json and -proto).
                includeGroupByRegex("com\\.google\\.firebase.*")
                includeGroupByRegex("androidx.*")
            }
        }
        mavenCentral()
        gradlePluginPortal()
    }
}

dependencyResolutionManagement {
    repositoriesMode = RepositoriesMode.FAIL_ON_PROJECT_REPOS
    repositories {
        google {
            content {
                includeGroupByRegex("com\\.android.*")
                includeGroupByRegex("com\\.google\\.(android|testing|prefab).*")
                // Play Billing's data transport needs the Firebase encoders, which only Google's
                // Maven hosts (com.google.firebase:firebase-encoders, -json and -proto).
                includeGroupByRegex("com\\.google\\.firebase.*")
                includeGroupByRegex("androidx.*")
            }
        }
        mavenCentral()
    }
}

rootProject.name = "jellyfesh-android"

fun androidSdkPresent(): Boolean {
    providers.gradleProperty("jellyfesh.android").orNull?.let { return it.toBoolean() }
    val env = providers.environmentVariable("ANDROID_HOME").orNull
        ?: providers.environmentVariable("ANDROID_SDK_ROOT").orNull
    if (!env.isNullOrBlank() && file(env).isDirectory) return true
    val local = file("local.properties")
    if (local.isFile) {
        val props = java.util.Properties()
        local.inputStream().use { props.load(it) }
        val dir = props.getProperty("sdk.dir")
        if (!dir.isNullOrBlank() && file(dir).isDirectory) return true
    }
    return false
}

val withAndroid = androidSdkPresent()
gradle.extra["jellyfesh.android"] = withAndroid

// CoreConfig.bundledPlugins is being added to the core and the FFI in this release. Until the
// generated bindings have it, :app and the JVM checks compile app/src/ffi-config-compat, which
// starts the core without the bundled plugins, instead of app/src/ffi-config.
val bindingsFile = file("core-bindings/src/main/kotlin/org/jellyfesh/ffi/jellyfesh.kt")
gradle.extra["jellyfesh.ffiConfigDir"] =
    if (bindingsFile.isFile && !bindingsFile.readText().contains("bundledPlugins")) "app/src/ffi-config-compat/kotlin" else "app/src/ffi-config/kotlin"

include(":logic")

if (file("core-bindings/src/main/kotlin/org/jellyfesh/ffi/jellyfesh.kt").isFile) {
    include(":ffi-check")
    if (file("core-bindings/src/main/kotlin/org/jellyfesh/tokens/Tokens.kt").isFile) include(":ui-check")
}

if (withAndroid) {
    include(":core-bindings", ":app")
}
