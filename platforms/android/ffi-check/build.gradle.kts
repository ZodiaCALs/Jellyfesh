// JVM check of the app's FFI adapter (app/src/ffi): compiled with the generated bindings and
// tokens (core-bindings/src/main/kotlin) and the shared drawing-time ports
// (platforms/shared/kotlin), and run against the jf-ffi library built for this machine. Never
// shipped; it exists so the code that names generated types is compiled and exercised without an
// Android SDK. Run: gradle :ffi-check:test (after `cargo xtask bindings --lang kotlin`).
import org.jetbrains.kotlin.gradle.dsl.JvmTarget

plugins {
    id("org.jetbrains.kotlin.jvm")
}

val androidDir: File = rootDir
val repoRoot: File = rootDir.resolve("../..").canonicalFile

// The host library the bindings load: JF_FFI_LIBRARY, else the cargo target folder.
val nativeLibrary: String = providers.environmentVariable("JF_FFI_LIBRARY").orNull ?: run {
    val target = providers.environmentVariable("CARGO_TARGET_DIR").orNull?.let { File(it) } ?: repoRoot.resolve("target")
    val name = when {
        System.getProperty("os.name").startsWith("Windows") -> "jf_ffi.dll"
        System.getProperty("os.name").startsWith("Mac") -> "libjf_ffi.dylib"
        else -> "libjf_ffi.so"
    }
    target.resolve("debug").resolve(name).path
}

java {
    sourceCompatibility = JavaVersion.VERSION_17
    targetCompatibility = JavaVersion.VERSION_17
}

kotlin {
    compilerOptions {
        jvmTarget = JvmTarget.JVM_17
    }
}

sourceSets {
    main {
        kotlin.srcDir(androidDir.resolve("core-bindings/src/main/kotlin"))
        kotlin.srcDir(repoRoot.resolve("platforms/shared/kotlin/src/main/kotlin"))
        kotlin.srcDir(androidDir.resolve("app/src/ffi/kotlin"))
        // CoreConfigs.kt with or without CoreConfig.bundledPlugins (settings.gradle.kts).
        kotlin.srcDir(androidDir.resolve(gradle.extra["jellyfesh.ffiConfigDir"] as String))
    }
}

dependencies {
    implementation(project(":logic"))
    implementation(libs.jna)

    testImplementation(kotlin("test"))
    testImplementation(libs.kotlinx.coroutines.test)
}

tasks.test {
    useJUnitPlatform()
    systemProperty("uniffi.component.jellyfesh.libraryOverride", nativeLibrary)
    systemProperty("jellyfesh.repoRoot", repoRoot.path)
    testLogging {
        events("passed", "skipped", "failed")
        exceptionFormat = org.gradle.api.tasks.testing.logging.TestExceptionFormat.FULL
    }
}
