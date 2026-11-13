package org.jellyfesh.android

import android.app.Application
import java.io.File
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.distinctUntilChanged
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import org.jellyfesh.android.ffi.PortDrawClock
import org.jellyfesh.android.ffi.UniffiCoreClient
import org.jellyfesh.android.images.ArtworkLoader
import org.jellyfesh.android.library.LibraryAccess
import org.jellyfesh.android.plugins.BundledPlugins
import org.jellyfesh.android.system.DeviceWatcher
import org.jellyfesh.android.system.IconSwitcher
import org.jellyfesh.logic.core.Command
import org.jellyfesh.logic.core.CoreClient
import org.jellyfesh.logic.core.CoreException
import org.jellyfesh.logic.nav.NavState
import org.jellyfesh.logic.state.CoreStore
import org.jellyfesh.logic.time.DrawClock
import org.jellyfesh.logic.time.MonotonicBridge

/** Whether the core has started. */
sealed interface CoreStartup {
    data object Starting : CoreStartup
    data object Ready : CoreStartup
    data class Failed(val message: String) : CoreStartup
}

/** The answer of a library query, as views show it. */
sealed interface QueryResult<out T> {
    data object Loading : QueryResult<Nothing>
    data class Ok<T>(val value: T) : QueryResult<T>
    /** The feature arrives later in this release (CoreFfiException.NotAvailable). */
    data class NotAvailable(val message: String) : QueryResult<Nothing>
    data class Failed(val message: String) : QueryResult<Nothing>
}

/** The value of a successful query, else null. */
fun <T> QueryResult<T>.valueOrNull(): T? = (this as? QueryResult.Ok<T>)?.value

/**
 * The app's long-lived parts: the core (started off the main thread), the store that mirrors it,
 * and the platform adapters. One per process, owned by [JellyfeshApp].
 */
class AppGraph(private val app: Application) {
    val scope = CoroutineScope(SupervisorJob() + Dispatchers.Main.immediate)
    val store = CoreStore()

    private val _startup = MutableStateFlow<CoreStartup>(CoreStartup.Starting)
    val startup: StateFlow<CoreStartup> = _startup.asStateFlow()

    @Volatile
    var client: CoreClient? = null
        private set

    @Volatile
    private var bridge: MonotonicBridge? = null

    /**
     * Where the person is in the UI. Kept here, for the life of the process, so the place
     * survives the activity being recreated (rotation, appearance or language changes).
     */
    val nav = MutableStateFlow(NavState())

    val images = ArtworkLoader(app, this)
    val icons = IconSwitcher(app)
    val library = LibraryAccess(app, this)
    private var devices: DeviceWatcher? = null

    /** Starts the core on a background thread; views show a placeholder until it is ready. */
    fun start() {
        scope.launch(Dispatchers.IO) {
            val started = try {
                UniffiCoreClient.start(
                    context = app.applicationContext,
                    dataDir = File(app.filesDir, "core").path,
                    cacheDir = File(app.cacheDir, "core").path,
                    bundledPlugins = BundledPlugins.prepare(app),
                )
            } catch (e: CoreException) {
                _startup.value = CoreStartup.Failed(e.message.orEmpty())
                return@launch
            } catch (e: UnsatisfiedLinkError) {
                _startup.value = CoreStartup.Failed(e.message.orEmpty())
                return@launch
            }
            bridge = MonotonicBridge(coreNow = started::monotonicNowNs, platformNow = System::nanoTime)
            store.attach(started)
            client = started
            _startup.value = CoreStartup.Ready
            withContext(Dispatchers.Main) {
                devices = DeviceWatcher(app) { send(Command.RefreshDevices) }.also { it.start() }
                followAppIcon()
            }
            send(Command.RefreshDevices)
            library.onCoreReady()
        }
    }

    /** Sends a command; 0 when the core is not running. */
    fun send(command: Command): Long = client?.send(command) ?: 0

    /** Now, on the clock the core stamps anchors with (see [MonotonicBridge]). */
    fun nowNs(): Long = bridge?.nowNs() ?: System.nanoTime()

    /** A frame time (`System.nanoTime` base) on the core's clock. */
    fun toCoreNs(platformNs: Long): Long = bridge?.toCore(platformNs) ?: platformNs

    /** A new ported `DisplayClock`, for one view tree. */
    fun newDrawClock(): DrawClock = PortDrawClock()

    /** Runs a synchronous core query on the IO dispatcher. */
    suspend fun <T> query(block: (CoreClient) -> T): QueryResult<T> = withContext(Dispatchers.IO) {
        val core = client ?: return@withContext QueryResult.Loading
        try {
            QueryResult.Ok(block(core))
        } catch (e: CoreException.NotAvailable) {
            QueryResult.NotAvailable(e.message.orEmpty())
        } catch (e: CoreException) {
            QueryResult.Failed(e.message.orEmpty())
        }
    }

    /** The launcher icon follows the app_icon setting (ADR-0017); the UI confirms before changing it. */
    private fun followAppIcon() {
        scope.launch {
            store.state.map { it.settings.appIcon }.distinctUntilChanged().collect { icons.apply(it) }
        }
    }
}
