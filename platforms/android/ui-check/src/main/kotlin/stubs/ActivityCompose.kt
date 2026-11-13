// Stand-ins for androidx.activity (activity-compose 1.13), same signatures as the real API.
// Bodies never run: :ui-check only compiles.
@file:Suppress("unused", "UNUSED_PARAMETER")

package androidx.activity.compose

import androidx.activity.result.ActivityResultLauncher
import androidx.activity.result.contract.ActivityResultContract
import androidx.compose.runtime.Composable

@Composable
fun BackHandler(enabled: Boolean = true, onBack: () -> Unit) {}

class ManagedActivityResultLauncher<I, O> internal constructor() : ActivityResultLauncher<I>() {
    override fun launch(input: I, options: androidx.core.app.ActivityOptionsCompat?) {}
    override fun unregister() {}
    override val contract: ActivityResultContract<I, *> get() = TODO()
}

@Composable
fun <I, O> rememberLauncherForActivityResult(
    contract: ActivityResultContract<I, O>,
    onResult: (O) -> Unit,
): ManagedActivityResultLauncher<I, O> = TODO()
