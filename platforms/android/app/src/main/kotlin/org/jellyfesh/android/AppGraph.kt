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
import org.jellyfesh.android.billing.PlayBilling
import org.jellyfesh.android.billing.StorePurchases
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
import org.jellyfesh.logic.core.Distribution
import org.jellyfesh.logic.nav.NavState
import org.jellyfesh.logic.player.ResumeOnLaunch
import org.jellyfesh.logic.settings.BuildFlags
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

    /** How this build reached the person (BuildConfig.DISTRIBUTION): the core decides from it where Pro is sold. */
    val distribution: Distribution = BuildFlags.distribution(BuildConfig.DISTRIBUTION)

    /**
     * Jellyfesh Pro in this build's store: Google Play Billing in the Google Play build, none in
     * the others. What the store says goes to the core, which decides whether Pro is on.
     */
    private val play: PlayBilling? = if (BuildFlags.usesPlayBilling(distribution)) {
        PlayBilling(app, BuildConfig.PLAY_LICENSE_KEY) { entitlement ->
            scope.launch(Dispatchers.IO) { runCatching { client?.setStoreEntitlement(entitlement) } }
        }
    } else {
        null
    }
    val purchases: StorePurchases? get() = play

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
                    distribution = distribution,
                    // The process may have started without its window (a media button, the system
                    // restarting the media service): the app resumes itself (ResumeOnLaunch).
                    resumeOnStart = false,
                )
            } catch (e: CoreException) {
                _startup.value = CoreStartup.Failed(e.message.orEmpty())
                return@launch
            } catch (e: UnsatisfiedLinkError) {
                _startup.value = CoreStartup.Failed(e.message.orEmpty())
                return@launch
            }
            bridge = MonotonicBridge(coreNow = started::monotonicNowNs, platformNow = System::nanoTime)
            // The core's word on source pictures goes straight to the artwork loader.
            store.artworkListener = images.covers::onReady
            store.attach(started)
            client = started
            _startup.value = CoreStartup.Ready
            withContext(Dispatchers.Main) {
                devices = DeviceWatcher(app) { send(Command.RefreshDevices) }.also { it.start() }
                followAppIcon()
                // What Google Play says about Pro, now that the core can hear it.
                play?.start()
                maybeResume()
            }
            send(Command.RefreshDevices)
            library.onCoreReady()
        }
    }

    /** When the process started (uptime), to tell a start for the window from one without it. */
    private val processStartMs: Long = android.os.Process.getStartUptimeMillis()

    /** Whether the process started for its window; null until the first window is created. */
    private var startedWithWindow: Boolean? = null
    private var resumeDecided = false

    /**
     * The activity's window was created ([uptimeMs]): the first one tells whether the process
     * started for it. Main thread.
     */
    fun windowCreated(uptimeMs: Long) {
        if (startedWithWindow == null) startedWithWindow = ResumeOnLaunch.startedWithWindow(processStartMs, uptimeMs)
        maybeResume()
    }

    /**
     * Resume on Launch, once per process: Play when the process started for its window and the
     * restored queue has a song (the core starts with `resumeOnStart` false). Main thread.
     */
    private fun maybeResume() {
        if (resumeDecided) return
        val window = startedWithWindow ?: return
        val state = store.state.value
        if (!state.ready) return
        resumeDecided = true
        ResumeOnLaunch.command(state, window)?.let(::send)
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
