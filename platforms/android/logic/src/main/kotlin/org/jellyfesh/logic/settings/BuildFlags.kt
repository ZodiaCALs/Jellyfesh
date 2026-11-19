package org.jellyfesh.logic.settings

import org.jellyfesh.logic.core.Distribution

/**
 * The build flags app/build.gradle.kts writes into BuildConfig (ADR-0021): the edition
 * (community or commercial) and the distribution (source, direct or google-play), read once.
 */
object BuildFlags {
    /** BuildConfig.DISTRIBUTION as the core's [Distribution]; anything unknown is a build from source. */
    fun distribution(value: String): Distribution = when (value.trim().lowercase()) {
        "direct" -> Distribution.Direct
        "google-play" -> Distribution.GooglePlay
        else -> Distribution.Source
    }

    /** Whether this build sells Pro through Google Play Billing (the Google Play App Bundle only). */
    fun usesPlayBilling(distribution: Distribution): Boolean = distribution == Distribution.GooglePlay
}
