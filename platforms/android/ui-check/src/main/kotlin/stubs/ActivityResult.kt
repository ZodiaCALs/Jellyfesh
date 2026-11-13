// Stand-ins for androidx.activity.result (activity 1.13). Bodies never run.
@file:Suppress("unused", "UNUSED_PARAMETER")

package androidx.activity.result

import androidx.activity.result.contract.ActivityResultContract

abstract class ActivityResultLauncher<I> {
    fun launch(input: I) = launch(input, null)
    abstract fun launch(input: I, options: androidx.core.app.ActivityOptionsCompat?)
    abstract fun unregister()
    abstract val contract: ActivityResultContract<I, *>
}
