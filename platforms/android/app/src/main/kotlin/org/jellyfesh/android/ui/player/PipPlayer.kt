package org.jellyfesh.android.ui.player

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.layout.onSizeChanged
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import org.jellyfesh.android.images.ArtKey
import org.jellyfesh.android.ui.components.ArtworkImage
import org.jellyfesh.android.ui.components.JfText
import org.jellyfesh.android.ui.theme.Jf
import org.jellyfesh.android.ui.theme.JfType
import org.jellyfesh.android.ui.theme.MiniProgressLine
import org.jellyfesh.android.ui.theme.MistyField
import org.jellyfesh.android.ui.theme.rounded
import org.jellyfesh.android.ui.time.LocalPlaybackTime
import org.jellyfesh.android.ui.time.TimeDemandEffect
import org.jellyfesh.logic.lyrics.LyricsBars
import org.jellyfesh.logic.player.PlayerModel
import org.jellyfesh.logic.seek.SeekGeometry
import org.jellyfesh.logic.state.CoreState
import org.jellyfesh.logic.time.TimeDemand
import org.jellyfesh.tokens.Tokens

/**
 * The mini player in Android's picture-in-picture window (ADR-0026 §4; docs/ui.md, "The player bar
 * and the mini player"): the cover, the title and the artist, the lyrics bar's line while Show
 * Lyrics Bar is on, and a progress line along the bottom that moves only by whole pixels. Its
 * controls are the media session's, which Android draws over the window when it is tapped.
 */
@Composable
fun PipPlayer(state: CoreState) {
    val colors = Jf.colors
    val time = LocalPlaybackTime.current
    val track = PlayerModel.track(state)
    val playing = PlayerModel.isPlaying(PlayerModel.status(state))
    val durationUs = PlayerModel.durationUs(state)
    val density = LocalDensity.current
    var steps by remember { mutableIntStateOf(300) }
    TimeDemandEffect(if (playing) TimeDemand.Steps(steps) else null)
    val lyricsLine = track != null &&
        LyricsBars.shows(state.settings.showLyricsBar, state.currentLyrics, PlayerModel.isRemote(state))
    Box(Modifier.fillMaxSize().onSizeChanged { steps = SeekGeometry.miniSteps(it.width.toFloat(), density.density) }) {
        MistyField(Modifier.fillMaxSize())
        if (track == null) return@Box
        Row(
            Modifier.fillMaxSize().padding(10.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(10.dp),
        ) {
            BoxWithConstraints {
                ArtworkImage(ArtKey.of(track.artwork), track.title, minOf(maxHeight, 96.dp), rounded(Tokens.Radius.rMd.dp))
            }
            Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(2.dp)) {
                JfText(track.title, style = JfType.listTitle.copy(fontWeight = FontWeight.SemiBold), maxLines = 1)
                track.artist?.let { JfText(it, style = JfType.listSecondary, color = colors.ink70, maxLines = 1) }
                if (lyricsLine) BarLyricsLine(state)
            }
        }
        val fraction = {
            val d = durationUs
            if (d == null || d <= 0) 0f else (time.positionUs.toFloat() / d).coerceIn(0f, 1f)
        }
        MiniProgressLine(fraction, Modifier.align(Alignment.BottomCenter).padding(bottom = 2.dp), inset = 10.dp)
    }
}
