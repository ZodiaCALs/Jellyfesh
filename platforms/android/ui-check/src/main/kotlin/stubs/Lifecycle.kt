// Stand-ins for androidx.lifecycle (lifecycle 2.11), which lives only on Google's Maven. Same
// names and signatures as the real API. Bodies never run.
@file:Suppress("unused", "UNUSED_PARAMETER")

package androidx.lifecycle

import kotlinx.coroutines.CoroutineScope

interface LifecycleObserver

abstract class Lifecycle {
    abstract val currentState: State

    abstract fun addObserver(observer: LifecycleObserver)

    abstract fun removeObserver(observer: LifecycleObserver)

    enum class State {
        DESTROYED, INITIALIZED, CREATED, STARTED, RESUMED;

        fun isAtLeast(state: State): Boolean = compareTo(state) >= 0
    }

    enum class Event { ON_CREATE, ON_START, ON_RESUME, ON_PAUSE, ON_STOP, ON_DESTROY, ON_ANY }
}

interface LifecycleOwner {
    val lifecycle: Lifecycle
}

suspend fun Lifecycle.repeatOnLifecycle(state: Lifecycle.State, block: suspend CoroutineScope.() -> Unit): Unit = TODO()

suspend fun LifecycleOwner.repeatOnLifecycle(state: Lifecycle.State, block: suspend CoroutineScope.() -> Unit): Unit = TODO()
