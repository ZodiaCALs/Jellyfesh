// Stand-in for the BuildConfig class the Android Gradle plugin generates for :app
// (app/build.gradle.kts: buildFeatures.buildConfig, the WEBSITE field). Never shipped; :ui-check
// only compiles, so the values are placeholders.
package org.jellyfesh.android

object BuildConfig {
    const val VERSION_NAME: String = "0.0.0"
    const val WEBSITE: String = ""
}
