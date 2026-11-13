package org.jellyfesh.android.ui.time

import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.Stable
import androidx.compose.runtime.State
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.derivedStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.staticCompositionLocalOf
import androidx.compose.runtime.withFrameNanos
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.compose.LocalLifecycleOwner
import androidx.lifecycle.repeatOnLifecycle
import kotlinx.coroutines.flow.distinctUntilChanged
import kotlinx.coroutines.flow.map
import org.jellyfesh.android.AppGraph
import org.jellyfesh.logic.time.PlaybackTicker
import org.jellyfesh.logic.time.TimeDemand

/**
 * Playback time for drawing (architecture section 5, "Drawing time without waste"): one
 * [PlaybackTicker] for the whole UI over the ported DisplayClock. It wakes only when something
 * visible changes (the next second, the next pixel of a bar, the next lyric change) and every
 * frame only while a word sweep moves; it stops with the activity (Still mode: nothing runs with
 * the screen off). Views read [positionUs] in their draw phase where they can, so a moving
 * position redraws pixels without recomposing.
 */
@Stable
class PlaybackTime internal constructor(val ticker: PlaybackTicker, private val position: State<Long>) {
    /** The position to draw, in microseconds. Read it in a draw phase or a derived state. */
    val positionUs: Long get() = position.value

    /** The position as State, for derivedStateOf. */
    val positionState: State<Long> get() = position
}

val LocalPlaybackTime = staticCompositionLocalOf<PlaybackTime> { error("PlaybackTime is missing") }

/** The ticker, fed with every anchor the core publishes and run while the activity is started. */
@Composable
fun rememberPlaybackTime(graph: AppGraph): PlaybackTime {
    val ticker = remember {
        PlaybackTicker(clock = graph.newDrawClock(), now = graph::nowNs, awaitFrame = { withFrameNanos { } })
    }
    val position = ticker.positionUs.collectAsState()
    LaunchedEffect(ticker) {
        graph.store.state.map { it.playback.anchor }.distinctUntilChanged().collect(ticker::onAnchor)
    }
    val lifecycle = LocalLifecycleOwner.current.lifecycle
    LaunchedEffect(ticker, lifecycle) {
        lifecycle.repeatOnLifecycle(Lifecycle.State.STARTED) { ticker.run() }
    }
    return remember(ticker, position) { PlaybackTime(ticker, position) }
}

/** Tells the ticker what this composable draws while it is on screen; null draws nothing timed. */
@Composable
fun TimeDemandEffect(demand: TimeDemand?) {
    val ticker = LocalPlaybackTime.current.ticker
    val key = remember { Any() }
    DisposableEffect(ticker, demand) {
        ticker.setDemand(key, demand)
        onDispose { ticker.setDemand(key, null) }
    }
}

/** The playback position in whole seconds, as State that changes once a second at most. */
@Composable
fun rememberPositionSeconds(): State<Long> {
    val time = LocalPlaybackTime.current
    return remember(time) { derivedStateOf { Math.floorDiv(time.positionUs, 1_000_000L) } }
}
