// Now Playing (design/canvas NowPlaying-Artwork and NowPlaying-Misty, controls D; after Apple
// Music in v0.0.4): the backdrop of the chosen style, crossfading between songs, a Veil bar with
// Library, the Lyrics/Queue/Info control and the Speakers chip, the player on the left and the
// chosen panel on the right. The artwork shrinks to 88 % while paused and springs back on play.

import SwiftUI

struct NowPlayingView: View {
    @Environment(SettingsStore.self) private var settings
    @Environment(Router.self) private var router
    @Environment(\.effects) private var effects

    var body: some View {
        let style = settings.current.style
        ZStack {
            NowPlayingBackdrop(style: style)
            VStack(spacing: 0) {
                NowPlayingBar()
                GeometryReader { geometry in
                    let wide = geometry.size.width >= 1100
                    HStack(alignment: .center, spacing: wide ? 56 : 36) {
                        PlayerColumn(style: style)
                            .frame(minWidth: 280, idealWidth: 400, maxWidth: 400)
                        panel
                            .frame(maxWidth: 640, maxHeight: .infinity)
                    }
                    .padding(.horizontal, wide ? 48 : 32)
                    .padding(.top, Tokens.Space.s6)
                    .padding(.bottom, Tokens.Space.s8)
                    .frame(width: geometry.size.width, height: geometry.size.height)
                }
            }
        }
        .onExitCommand {
            router.showNowPlaying(false, reduceMotion: !effects.animates)
        }
        .accessibilityElement(children: .contain)
        .accessibilityLabel(Text("Now Playing"))
    }

    @ViewBuilder private var panel: some View {
        switch router.panel {
        case .lyrics: LyricsPanel()
        case .queue: QueuePanel()
        case .info: InfoPanel()
        }
    }
}

/// The blurred artwork, baked by the core and tone-mapped for contrast (Artwork style, deviation
/// D1), crossfading on track change; the Misty field at the artwork hue otherwise, and while the
/// track has no artwork.
struct NowPlayingBackdrop: View {
    var style: BackdropOption

    @Environment(ThemeStore.self) private var theme
    @Environment(\.colorScheme) private var colorScheme
    @Environment(\.effects) private var effects
    @Environment(\.artworkLoader) private var loader
    @State private var image: NSImage?
    @State private var shownPath: String?

    var body: some View {
        let path = style == .artwork ? theme.backdrop(dark: colorScheme == .dark) : nil
        ZStack {
            MistyField()
            if let image, shownPath != nil {
                Image(nsImage: image)
                    .resizable()
                    .scaledToFill()
                    .id(shownPath)
                    .transition(.opacity)
            }
        }
        .ignoresSafeArea()
        .accessibilityHidden(true)
        .task(id: path) {
            var loaded: NSImage?
            if let path, let loader {
                loaded = await loader.image(.file(path), pixels: 512)
            }
            guard !Task.isCancelled else { return }
            let show = {
                image = loaded
                shownPath = loaded == nil ? nil : path
            }
            if effects.crossfadesBackdrop {
                withAnimation(Motion.themeChange, show)
            } else {
                show()
            }
        }
    }
}

/// The Veil bar at the top of Now Playing (control style D).
struct NowPlayingBar: View {
    @Environment(Router.self) private var router
    @Environment(\.palette) private var palette
    @Environment(\.effects) private var effects

    var body: some View {
        @Bindable var router = router
        HStack(spacing: Tokens.Space.s4) {
            Button {
                router.showNowPlaying(false, reduceMotion: !effects.animates)
            } label: {
                HStack(spacing: Tokens.Space.s1) {
                    Image(systemName: "chevron.left")
                        .font(.system(size: 14, weight: .semibold))
                    Text("Library")
                        .font(.system(size: 13, weight: .semibold))
                }
                .foregroundStyle(palette[Tokens.Ink.ink70])
                .padding(.horizontal, 10)
                .frame(height: 40)
                .contentShape(Rectangle())
            }
            .buttonStyle(.plain)
            .keyboardShortcut(.escape, modifiers: [])
            .accessibilityLabel(Text("Back to Library"))
            .help(Text("Back to Library"))

            Spacer(minLength: 0)

            SegmentedControl(
                selection: $router.panel,
                options: [
                    .init(value: .lyrics, title: "Lyrics"),
                    .init(value: .queue, title: "Queue"),
                    .init(value: .info, title: "Info"),
                ],
                label: "Now Playing views"
            )

            Spacer(minLength: 0)

            OutputDeviceMenu()
        }
        .padding(.horizontal, Tokens.Space.s3)
        .frame(minHeight: 56)
        .glass(.veil, in: Rectangle(), edge: false)
        .overlay(alignment: .bottom) {
            Rectangle()
                .fill(palette[Tokens.Glass.glassEdge].opacity(0.4))
                .frame(height: 1)
        }
    }
}

/// Artwork, title and artist, the seek bar, the transport and the volume.
struct PlayerColumn: View {
    var style: BackdropOption

    @Environment(PlaybackStore.self) private var playback
    @Environment(LibraryStore.self) private var library
    @Environment(\.palette) private var palette
    @Environment(\.effects) private var effects
    @State private var favorite: Bool?

    var body: some View {
        let now = playback.nowPlaying
        let playing = playback.isPlaying
        VStack(alignment: .leading, spacing: Tokens.Space.s5) {
            artwork(now)
                .scaleEffect(NowPlayingMotion.artworkScale(playing: playing, reduceMotion: !effects.animates))
                .animation(effects.animates ? (playing ? Motion.artworkScale : Motion.fluid(Tokens.Motion.dur3Ms)) : nil,
                           value: playing)
                .frame(maxWidth: 400, maxHeight: 400)
                .frame(maxWidth: .infinity)
            HStack(alignment: .top, spacing: Tokens.Space.s3) {
                VStack(alignment: .leading, spacing: Tokens.Space.s1) {
                    Text(now?.title ?? "")
                        .typography(Tokens.TypeRoles.nowPlayingTitle)
                        .foregroundStyle(palette[Tokens.Ink.ink90])
                        .lineLimit(2)
                        .accessibilityAddTraits(.isHeader)
                    Text(CountText.joined([now?.artist, now?.album]))
                        .typography(Tokens.TypeRoles.nowPlayingArtist)
                        .foregroundStyle(palette[Tokens.Ink.ink70])
                        .lineLimit(2)
                }
                Spacer(minLength: 0)
                if let track = now?.track, case .library = track {
                    Button {
                        library.setFavorite(track, !(favorite ?? false))
                    } label: {
                        Image(systemName: favorite == true ? "heart.fill" : "heart")
                    }
                    .buttonStyle(ToggleButtonStyle(isOn: favorite == true))
                    .accessibilityLabel(favorite == true ? Text("Remove from Favourites") : Text("Add to Favourites"))
                    .help(favorite == true ? Text("Remove from Favourites") : Text("Add to Favourites"))
                }
            }
            SeekBar(liquidThumb: style == .artwork)
            TransportRow()
            VolumeSlider()
        }
        .task(id: FavoriteKey(track: now?.track, revision: library.revision)) {
            guard let track = now?.track else {
                favorite = nil
                return
            }
            favorite = try? await library.client.isFavorite(track)
        }
    }

    private struct FavoriteKey: Equatable {
        var track: TrackReference?
        var revision: UInt64
    }

    @ViewBuilder private func artwork(_ now: NowPlayingInfo?) -> some View {
        let source = now?.artwork.flatMap { ArtworkSource(hash: nil, location: $0) }
        if style == .misty {
            // Misty style: the artwork is hero media on a Clear frame.
            ArtworkView(source: source, size: nil, cornerRadius: Tokens.Artwork.radiusHero)
                .padding(Tokens.Space.s3)
                .glass(.clear, in: RoundedRectangle(cornerRadius: Tokens.Artwork.radiusHero + Tokens.Space.s3,
                                                    style: .continuous))
                .elevation(Tokens.Elevation.shadow1)
        } else {
            ArtworkView(source: source, size: nil, cornerRadius: Tokens.Artwork.radiusHero)
                .elevation(Tokens.Elevation.shadow3)
        }
    }
}

/// Shuffle, the transport capsule and repeat (control style D).
struct TransportRow: View {
    @Environment(PlaybackStore.self) private var playback

    var body: some View {
        HStack(spacing: Tokens.Space.s4) {
            Button {
                playback.toggleShuffle()
            } label: {
                Image(systemName: "shuffle")
            }
            .buttonStyle(ToggleButtonStyle(isOn: playback.queue.shuffle))
            .accessibilityLabel(Text("Shuffle"))
            .accessibilityValue(playback.queue.shuffle ? Text("On") : Text("Off"))
            .help(Text("Shuffle"))

            TransportCapsule {
                Button {
                    playback.previous()
                } label: {
                    Image(systemName: "backward.end")
                }
                .buttonStyle(IconButtonStyle(size: 52, glyph: 22, prominent: true))
                .accessibilityLabel(Text("Previous"))
                .help(Text("Previous"))

                PlayPauseButton(size: 60, glyph: 24, prominent: true)

                Button {
                    playback.next()
                } label: {
                    Image(systemName: "forward.end")
                }
                .buttonStyle(IconButtonStyle(size: 52, glyph: 22, prominent: true))
                .accessibilityLabel(Text("Next"))
                .help(Text("Next"))
            }

            Button {
                playback.cycleRepeat()
            } label: {
                Image(systemName: playback.queue.repeatMode == .one ? "repeat.1" : "repeat")
            }
            .buttonStyle(ToggleButtonStyle(isOn: playback.queue.repeatMode != .off))
            .accessibilityLabel(Text("Repeat"))
            .accessibilityValue(Text(repeatText))
            .help(Text("Repeat: \(repeatText)"))
        }
        .frame(maxWidth: .infinity)
    }

    private var repeatText: String {
        switch playback.queue.repeatMode {
        case .off: return String(localized: "Off")
        case .all: return String(localized: "All")
        case .one: return String(localized: "One")
        }
    }
}
