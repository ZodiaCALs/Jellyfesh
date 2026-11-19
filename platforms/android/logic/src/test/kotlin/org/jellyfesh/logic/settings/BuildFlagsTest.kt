package org.jellyfesh.logic.settings

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertTrue
import org.jellyfesh.logic.core.Distribution

class BuildFlagsTest {
    @Test
    fun theBuildsDistributionReachesTheCore() {
        assertEquals(Distribution.Direct, BuildFlags.distribution("direct"))
        assertEquals(Distribution.GooglePlay, BuildFlags.distribution(" Google-Play "))
        assertEquals(Distribution.Source, BuildFlags.distribution("source"))
        assertEquals(Distribution.Source, BuildFlags.distribution(""), "a build that says nothing is from source")
        assertTrue(BuildFlags.usesPlayBilling(Distribution.GooglePlay))
        assertFalse(BuildFlags.usesPlayBilling(Distribution.Direct))
    }
}
