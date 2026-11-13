// The core for Android: the UniFFI Kotlin bindings and design tokens that
// `cargo xtask bindings --lang kotlin` and `cargo xtask tokens --target kotlin` write into
// src/main/kotlin (git-ignored), the shared drawing-time ports from platforms/shared/kotlin, JNA,
// and libjf_ffi.so for each ABI, built by cargo-ndk in [CargoNdkBuild].
import javax.inject.Inject
import org.gradle.process.ExecOperations

plugins {
    id("com.android.library")
}

val repoRoot: File = rootDir.resolve("../..").canonicalFile
val coreCrate: String = providers.gradleProperty("jellyfesh.coreCrate").getOrElse("jf-ffi")
val coreAbis = listOf("arm64-v8a", "x86_64")
val minApi = 26

android {
    namespace = "org.jellyfesh.core"
    compileSdk = 37

    defaultConfig {
        minSdk = minApi
        consumerProguardFiles("consumer-rules.pro")
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }
}

androidComponents {
    onVariants { variant ->
        // DisplayClock and the lyric cursor, ported once for every Kotlin shell (docs/ffi.md).
        variant.sources.kotlin?.addStaticSourceDirectory(repoRoot.resolve("platforms/shared/kotlin/src/main/kotlin").path)
    }
}

/**
 * Builds the core's shared library for each ABI with cargo-ndk (`cargo install cargo-ndk`; the
 * NDK through ANDROID_NDK_HOME or the SDK's ndk/ folder; CMake for libopus). Cargo is
 * incremental, so the task always runs and finishes at once when nothing changed.
 */
abstract class CargoNdkBuild @Inject constructor(private val exec: ExecOperations) : DefaultTask() {
    @get:Input abstract val crate: Property<String>
    @get:Input abstract val abis: ListProperty<String>
    @get:Input abstract val platform: Property<Int>
    @get:Input abstract val release: Property<Boolean>
    @get:Internal abstract val workspace: DirectoryProperty
    @get:OutputDirectory abstract val outputDir: DirectoryProperty

    init {
        outputs.upToDateWhen { false }
    }

    @TaskAction
    fun build() {
        val args = mutableListOf("ndk")
        abis.get().forEach { args += listOf("-t", it) }
        args += listOf("--platform", platform.get().toString(), "-o", outputDir.get().asFile.path, "build", "-p", crate.get())
        if (release.get()) args += "--release"
        exec.exec {
            workingDir = workspace.get().asFile
            commandLine(listOf("cargo") + args)
        }
    }
}

val cargoNdkBuild = tasks.register<CargoNdkBuild>("cargoNdkBuild") {
    description = "Builds $coreCrate for ${coreAbis.joinToString()} with cargo-ndk."
    crate.set(coreCrate)
    abis.set(coreAbis)
    platform.set(minApi)
    release.set(providers.gradleProperty("jellyfesh.coreDebug").map { !it.toBoolean() }.orElse(true))
    workspace.set(repoRoot)
    outputDir.set(layout.buildDirectory.dir("rustJniLibs"))
}

// -Pjellyfesh.skipCargo=true packages libraries already in src/main/jniLibs instead.
if (!providers.gradleProperty("jellyfesh.skipCargo").map { it.toBoolean() }.getOrElse(false)) {
    androidComponents {
        onVariants { variant ->
            variant.sources.jniLibs?.addGeneratedSourceDirectory(cargoNdkBuild, CargoNdkBuild::outputDir)
        }
    }
}

dependencies {
    api(project(":logic"))
    // JNA with its Android native part (libjnidispatch.so per ABI); the bindings load jf_ffi with it.
    api("net.java.dev.jna:jna:${libs.versions.jna.get()}@aar")
}
