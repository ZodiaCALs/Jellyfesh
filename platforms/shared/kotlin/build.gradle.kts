import org.jetbrains.kotlin.gradle.dsl.JvmTarget

// Versions checked against the Gradle plugin portal and Maven Central on 2026-11-07.
plugins {
    kotlin("jvm") version "2.4.21"
}

val repoRoot: File = rootDir.resolve("../../..").canonicalFile

// The UniFFI bindings and tokens that `cargo xtask bindings --lang kotlin` and
// `cargo xtask tokens --target kotlin` write; the ports use their types.
val generatedKotlin: File = repoRoot.resolve("platforms/android/core-bindings/src/main/kotlin")

// The jf-ffi library the bindings load (built by `cargo xtask bindings`); JF_FFI_LIBRARY
// overrides it.
val nativeLibrary: String = providers.environmentVariable("JF_FFI_LIBRARY").getOrElse(
    repoRoot.resolve("target/debug").resolve(
        when {
            System.getProperty("os.name").startsWith("Windows") -> "jf_ffi.dll"
            System.getProperty("os.name").startsWith("Mac") -> "libjf_ffi.dylib"
            else -> "libjf_ffi.so"
        },
    ).path,
)

kotlin {
    // Android's target; the ports are plain Kotlin.
    compilerOptions.jvmTarget.set(JvmTarget.JVM_17)
}

sourceSets {
    main {
        kotlin.srcDir(generatedKotlin)
    }
    test {
        // Only once `cargo xtask tokens --target kotlin` has written them.
        if (!generatedKotlin.resolve("org/jellyfesh/tokens/Tokens.kt").exists()) {
            kotlin.exclude("**/TokensTest.kt")
        }
    }
}

java {
    sourceCompatibility = JavaVersion.VERSION_17
    targetCompatibility = JavaVersion.VERSION_17
}

dependencies {
    implementation("net.java.dev.jna:jna:5.19.1")
    testImplementation("org.junit.jupiter:junit-jupiter:6.1.3")
    testImplementation("org.jetbrains.kotlinx:kotlinx-serialization-json:1.12.0")
    testRuntimeOnly("org.junit.platform:junit-platform-launcher:6.1.3")
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
