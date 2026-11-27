// The Jellyfesh Android app: Compose UI, Media3 session, platform adapters. Business logic lives in
// the core; the shell's own testable parts are in :logic, and the FFI adapter in src/ffi (also
// compiled on the JVM by :ffi-check).
plugins {
    id("com.android.application")
    id("org.jetbrains.kotlin.plugin.compose")
}

val repoRoot: File = rootDir.resolve("../..").canonicalFile

/**
 * The website Settings › About links to, with the third-party notices it publishes
 * (THIRD_PARTY_NOTICES.html, tools/release/build_site.py): the `jellyfesh.website` Gradle property,
 * else, in GitHub Actions, the repository's GitHub Pages address, where web.yml deploys the website
 * (docs/github.md). Without either, About shows the version only.
 */
val website: String = providers.gradleProperty("jellyfesh.website").orNull?.trim()?.takeIf { it.isNotEmpty() }
    ?: providers.environmentVariable("GITHUB_REPOSITORY").orNull?.takeIf { it.count { c -> c == '/' } == 1 }?.let { repository ->
        val (owner, name) = repository.split('/')
        "https://${owner.lowercase()}.github.io/$name/"
    }
    ?: ""

/**
 * A build flag (ADR-0021, docs/github.md): the Gradle property, else the environment variable (the
 * release workflow sets those), else the default.
 */
fun buildFlag(property: String, environment: String, default: String): String =
    providers.gradleProperty(property).orNull?.trim()?.takeIf { it.isNotEmpty() }
        ?: providers.environmentVariable(environment).orNull?.trim()?.takeIf { it.isNotEmpty() }
        ?: default

/** community (builds from source) or commercial (release builds: no unofficial community plugins bundled). */
val edition: String = buildFlag("jellyfesh.edition", "JF_EDITION", "community")

/** source (builds from source), direct (the APK on the release page) or google-play (the App Bundle). */
val distribution: String = buildFlag("jellyfesh.distribution", "JF_DISTRIBUTION", "source")

/**
 * The app's licence key from Play Console (Monetize with Play › Monetization setup): the public key
 * that checks the signatures of Play purchases. Not a secret; empty when not given.
 */
val playLicenseKey: String = buildFlag("jellyfesh.playLicenseKey", "PLAY_LICENSE_KEY", "")

require(edition in setOf("community", "commercial")) {
    "jellyfesh.edition (JF_EDITION) must be community or commercial, not '$edition'"
}
require(distribution in setOf("source", "direct", "google-play")) {
    "jellyfesh.distribution (JF_DISTRIBUTION) must be source, direct or google-play, not '$distribution'"
}

/** A Kotlin and Java string literal for a BuildConfig field. */
fun javaString(value: String): String = "\"" + value.replace("\\", "\\\\").replace("\"", "\\\"") + "\""

// CoreConfigs.kt with or without CoreConfig.bundledPlugins, as the generated bindings allow.
val ffiConfigDir = gradle.extra["jellyfesh.ffiConfigDir"] as String
if (ffiConfigDir.endsWith("-compat/kotlin")) {
    logger.warn("w: the generated bindings have no CoreConfig.bundledPlugins yet: the app starts the core without its bundled plugins")
}

android {
    namespace = "org.jellyfesh.android"
    compileSdk = 37

    defaultConfig {
        applicationId = "org.jellyfesh.android"
        minSdk = 26
        targetSdk = 37
        versionCode = 8
        versionName = "0.0.8"
        // Settings › About shows versionName (BuildConfig.VERSION_NAME) and links to the website.
        buildConfigField("String", "WEBSITE", "\"${website.replace("\\", "\\\\").replace("\"", "\\\"")}\"")
        // The edition and distribution for the app's store glue and Settings (ADR-0021):
        // BuildConfig.EDITION, BuildConfig.DISTRIBUTION and BuildConfig.PLAY_LICENSE_KEY.
        buildConfigField("String", "EDITION", javaString(edition))
        buildConfigField("String", "DISTRIBUTION", javaString(distribution))
        buildConfigField("String", "PLAY_LICENSE_KEY", javaString(playLicenseKey))
        ndk {
            abiFilters += listOf("arm64-v8a", "x86_64")
        }
    }

    buildTypes {
        release {
            // Unsigned: the release workflow signs the APK (apksigner) and the App Bundle
            // (jarsigner) with the release keystore afterwards (docs/store-release.md).
            isMinifyEnabled = true
            isShrinkResources = true
            proguardFiles(getDefaultProguardFile("proguard-android-optimize.txt"), "proguard-rules.pro")
        }
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }

    buildFeatures {
        compose = true
        // VERSION_NAME and WEBSITE for Settings › About (AGP turns BuildConfig off by default).
        buildConfig = true
    }

    lint {
        // The first CI builds of this never-compiled shell should report lint findings, not stop
        // the APK; tighten once it has run on devices.
        checkReleaseBuilds = false
        abortOnError = false
    }

    packaging {
        resources.excludes += listOf("META-INF/AL2.0", "META-INF/LGPL2.1")
    }

    androidResources {
        // The bundled plugins are zip files already.
        noCompress += "jfpkg"
    }
}

/**
 * Packs the official plugins (the .jfpkg files in dist/plugins, made by the plugin packaging
 * script) into the APK's assets/plugins; the app hands them to the core as bundled plugins.
 * Without them the APK builds anyway, with a warning, and ships no bundled plugins. The commercial
 * edition leaves out the unofficial community plugins, which stay separate downloads (ADR-0021 §4).
 */
abstract class BundlePlugins : DefaultTask() {
    @get:InputFiles
    @get:PathSensitive(PathSensitivity.NAME_ONLY)
    abstract val packages: ConfigurableFileCollection

    @get:OutputDirectory
    abstract val outputDir: DirectoryProperty

    @TaskAction
    fun copy() {
        val out = outputDir.get().asFile.resolve("plugins")
        out.deleteRecursively()
        out.mkdirs()
        val files = packages.files.filter { it.isFile && it.name.endsWith(".jfpkg") }.sortedBy { it.name }
        if (files.isEmpty()) {
            logger.warn("w: no official plugins in dist/plugins; the APK ships without bundled plugins (package them first, see docs/android.md)")
        }
        files.forEach { it.copyTo(out.resolve(it.name), overwrite = true) }
    }
}

val bundlePlugins = tasks.register<BundlePlugins>("bundlePlugins") {
    description = "Copies dist/plugins/*.jfpkg into the APK's assets/plugins."
    packages.from(
        fileTree(repoRoot.resolve("dist/plugins")) {
            include("*.jfpkg")
            if (edition == "commercial") exclude("community.unofficial.*")
        },
    )
    outputDir.set(layout.buildDirectory.dir("generated/bundledPlugins"))
}

androidComponents {
    onVariants { variant ->
        // src/ffi holds the only code that touches the generated bindings (docs/android.md).
        // The variant API (stable since AGP 8) adds it, rather than the source-set DSL.
        variant.sources.kotlin?.addStaticSourceDirectory("src/ffi/kotlin")
        variant.sources.kotlin?.addStaticSourceDirectory(ffiConfigDir.removePrefix("app/"))
        variant.sources.assets?.addGeneratedSourceDirectory(bundlePlugins, BundlePlugins::outputDir)
    }
}

dependencies {
    implementation(project(":core-bindings"))
    implementation(project(":logic"))

    implementation(platform(libs.compose.bom))
    implementation(libs.compose.foundation)
    implementation(libs.compose.ui)
    implementation(libs.compose.animation)
    implementation(libs.activity.compose)
    implementation(libs.androidx.core.ktx)
    implementation(libs.lifecycle.runtime.compose)
    implementation(libs.media3.session)
    implementation(libs.media3.common)
    implementation(libs.browser)
    implementation(libs.cloudy)
    implementation(libs.kotlinx.coroutines.android)
    // Jellyfesh Pro in Google Play builds (ADR-0021); unused elsewhere.
    implementation(libs.play.billing)
}
