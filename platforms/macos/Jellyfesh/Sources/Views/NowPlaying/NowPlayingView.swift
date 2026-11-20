// Now Playing as a stage (Luminous, docs/ui.md): the backdrop of the chosen style behind
// everything (the core's Artwork bake by default), the artwork at 380 pt on shadow-3 with the title
// (`type-roles.now-playing-title`) and the artist under it, the chosen pane (Lyrics, Queue, Info) on
// the right, and every control in one floating Veil dock: shuffle, previous, play, next, repeat,
// the scrubber with its times, the volume, and on the right the device button and More. Library
// and the Lyrics/Queue/Info control float at the top. The artwork shrinks to 88 % while paused and
// springs back on play. While a remote session runs it shows the session's track (ADR-0018). A
// customized look may lay it out as a large cover with the line being sung below it (Artwork) or
// as the pane across the window with a small cover in the corner (Lyrics; ADR-0020).

import SwiftUI

struct NowPlayingView: View {
    @Environment(SettingsStore.self) private var settings
    @Environment(Router.self) private var router
    @Environment(\.effects) private var effects
    @Environment(\.look) private var look

    var body: some View {
        let style = settings.current.style
        ZStack {
            NowPlayingBackdrop(style: style)
            GeometryReader { geometry in
                let width = geometry.size.width
                let height = geometry.size.height
                let wide = width >= 1100
                VStack(spacing: 0) {
                    NowPlayingBar()
                    switch LuminousGeometry.nowPlayingArrangement(look.nowPlaying, panel: router.panel) {
                    case .stage:
                        let side = LuminousGeometry.stageArtworkSide(width: width, height: height)
                        HStack(alignment: .center, spacing: wide ? 64 : 40) {
                            StageArtwork(style: style, side: side)
                                .frame(width: side)
                            panel
                                .frame(maxWidth: .infinity, maxHeight: .infinity)
                        }
                        .padding(.horizontal, wide ? 96 : 40)
                        .padding(.top, Tokens.Space.s2)
                        .padding(.bottom, Tokens.Space.s5)
                    case .cover:
                        let side = LuminousGeometry.coverArtworkSide(width: width, height: height)
                        VStack(spacing: Tokens.Space.s5) {
                            Spacer(minLength: 0)
                            StageArtwork(style: style, side: side)
                                .frame(width: side)
                            NowPlayingSungLine(fontSize: 26)
                                .frame(maxWidth: 900)
                            Spacer(minLength: 0)
                        }
                        .frame(maxWidth: .infinity, maxHeight: .infinity)
                        .padding(.horizontal, 40)
                        .padding(.bottom, Tokens.Space.s5)
                    case .widePane:
                        HStack(alignment: .top, spacing: wide ? 48 : 32) {
                            CornerCover()
                            panel
                                .frame(maxWidth: .infinity, maxHeight: .infinity)
                        }
                        .padding(.horizontal, wide ? 64 : 32)
                        .padding(.top, Tokens.Space.s2)
                        .padding(.bottom, Tokens.Space.s5)
                    }
                    NowPlayingDock()
                        .padding(.horizontal, wide ? 96 : 32)
                        .padding(.bottom, 28)
                }
                .frame(width: width, height: geometry.size.height)
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
        case .comments: CommentsPanel()
        }
    }
}

/// The blurred artwork, baked by the core and tone-mapped for contrast (Artwork style, deviation
/// D1), crossfading on track change; the Luminous field at the artwork hue otherwise, and while
/// the track has no artwork.
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

/// Library and the Lyrics/Queue/Info control, floating at the top of the stage.
struct NowPlayingBar: View {
    @Environment(Router.self) private var router
    @Environment(PlaybackStore.self) private var playback
    @Environment(SourcesStore.self) private var sources
    @Environment(\.palette) private var palette
    @Environment(\.effects) private var effects

    var body: some View {
        @Bindable var router = router
        let comments = CommentsModel.item(for: playback.nowPlaying?.track, remote: playback.isRemote,
                                          showsComments: { sources.plugin($0)?.hasComments == true }) != nil
        ZStack {
            SegmentedControl(
                selection: $router.panel,
                options: Self.panes(comments: comments),
                label: "Now Playing views"
            )
            .onChange(of: comments) { _, shown in
                // A song whose source shows no comments goes back to its lyrics.
                if !shown && router.panel == .comments { router.panel = .lyrics }
            }
            .padding(3)
            .glass(.veil, in: Capsule())
            .elevation(Tokens.Elevation.shadow1)

            HStack {
                Button {
                    router.showNowPlaying(false, reduceMotion: !effects.animates)
                } label: {
                    HStack(spacing: Tokens.Space.s1) {
                        Image(systemName: "chevron.left")
                            .font(.system(size: 14, weight: .semibold))
                        Text("Library")
                            .lookFont(size: 13, weight: .semibold)
                    }
                    .foregroundStyle(palette[Tokens.Ink.ink90])
                    .padding(.horizontal, 14)
                    .frame(height: 40)
                    .glass(.veil, in: Capsule())
                    .elevation(Tokens.Elevation.shadow1)
                    .contentShape(Capsule())
                }
                .buttonStyle(.plain)
                .keyboardShortcut(.escape, modifiers: [])
                .accessibilityLabel(Text("Back to Library"))
                .help(Text("Back to Library"))
                Spacer(minLength: 0)
            }
        }
        .padding(.horizontal, 28)
        .frame(height: 78)
    }

    /// Lyrics, Queue and Info, and Comments where the song's source shows them.
    private static func panes(comments: Bool) -> [SegmentedControl<NowPlayingPanel>.Option] {
        var panes: [SegmentedControl<NowPlayingPanel>.Option] = [
            .init(value: .lyrics, title: "Lyrics"),
            .init(value: .queue, title: "Queue"),
            .init(value: .info, title: "Info"),
        ]
        if comments {
            panes.append(.init(value: .comments, title: "Comments"))
        }
        return panes
    }
}

/// The Lyrics layout's small cover in the corner, with the title and the artist under it.
private struct CornerCover: View {
    @Environment(PlaybackStore.self) private var playback
    @Environment(\.palette) private var palette

    var body: some View {
        let item = playback.playing
        VStack(alignment: .leading, spacing: Tokens.Space.s2) {
            ArtworkView(source: item?.artwork, size: 120, cornerRadius: Tokens.Artwork.radiusGrid)
                .elevation(Tokens.Elevation.shadow2)
            Text(item?.title ?? "")
                .typography(Tokens.TypeRoles.listTitle)
                .foregroundStyle(palette[Tokens.Ink.ink90])
                .lineLimit(2)
                .accessibilityAddTraits(.isHeader)
            Text(CountText.joined([item?.artist, item?.album]))
                .typography(Tokens.TypeRoles.listSecondary)
                .foregroundStyle(palette[Tokens.Ink.ink70])
                .lineLimit(2)
        }
        .frame(width: 140, alignment: .leading)
        .padding(.top, Tokens.Space.s4)
    }
}

/// The artwork and what plays: title, artist and album, and for a remote session where it plays.
struct StageArtwork: View {
    var style: BackdropOption
    var side: CGFloat

    @Environment(PlaybackStore.self) private var playback
    @Environment(LibraryStore.self) private var library
    @Environment(\.palette) private var palette
    @Environment(\.effects) private var effects
    @Environment(\.look) private var look
    @State private var favorite: Bool?

    var body: some View {
        let item = playback.playing
        let playing = playback.isPlaying
        VStack(alignment: .leading, spacing: Tokens.Space.s5) {
            artwork(item)
                .frame(width: side, height: side)
                .scaleEffect(NowPlayingMotion.artworkScale(playing: playing, reduceMotion: !effects.animates))
                .animation(effects.animates ? (playing ? Motion.artworkScale : Motion.fluid(Tokens.Motion.dur3Ms)) : nil,
                           value: playing)
            HStack(alignment: .top, spacing: Tokens.Space.s3) {
                VStack(alignment: .leading, spacing: Tokens.Space.s1) {
                    Text(item?.title ?? "")
                        .typography(Tokens.TypeRoles.nowPlayingTitle)
                        .foregroundStyle(palette[Tokens.Ink.ink90])
                        .lineLimit(2)
                        .accessibilityAddTraits(.isHeader)
                    Text(CountText.joined([item?.artist, item?.album]))
                        .typography(Tokens.TypeRoles.nowPlayingArtist)
                        .foregroundStyle(palette[Tokens.Ink.ink70])
                        .lineLimit(2)
                    if let remote = item?.remote {
                        Text(RemoteText.playingOn(remote))
                            .lookFont(size: 13, weight: .semibold)
                            .foregroundStyle(palette[Tokens.Ink.ink70])
                            .lineLimit(1)
                        if let error = remote.error {
                            Text(error)
                                .lookFont(size: 12)
                                .foregroundStyle(palette[Tokens.Ink.ink50])
                                .lineLimit(2)
                        }
                    }
                }
                Spacer(minLength: 0)
                if let track = item?.track, case .library = track {
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
        }
        .task(id: FavoriteKey(track: item?.track, revision: library.revision)) {
            guard let track = item?.track else {
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

    @ViewBuilder private func artwork(_ item: PlayingItem?) -> some View {
        let source = item?.artwork
        if style == .misty {
            // Misty style: the artwork is hero media on a Clear frame.
            ArtworkView(source: source, size: side - 2 * Tokens.Space.s3, cornerRadius: Tokens.Artwork.radiusHero)
                .padding(Tokens.Space.s3)
                .glass(.clear, in: RoundedRectangle(cornerRadius: look.radius(Tokens.Artwork.radiusHero + Tokens.Space.s3),
                                                    style: .continuous))
                .elevation(Tokens.Elevation.shadow1)
        } else {
            ArtworkView(source: source, size: side, cornerRadius: Tokens.Artwork.radiusHero)
                .elevation(Tokens.Elevation.shadow3)
        }
    }
}

/// Every control in one floating Veil capsule (Luminous): shuffle, previous, play, next and repeat,
/// the scrubber with its times, the volume (hidden for a remote device without one), and the
/// device button and More.
struct NowPlayingDock: View {
    var body: some View {
        FloatingGlass(cornerRadius: LuminousGeometry.dockRadius) {
            DockControls()
        }
        .elevation(Tokens.Elevation.shadow2)
        .frame(maxWidth: 1180)
    }
}

private struct DockControls: View {
    @Environment(PlaybackStore.self) private var playback
    @Environment(SettingsStore.self) private var settings

    var body: some View {
        HStack(spacing: Tokens.Space.s5) {
            HStack(spacing: Tokens.Space.s2) {
                Button {
                    playback.toggleShuffle()
                } label: {
                    Image(systemName: "shuffle")
                }
                .buttonStyle(ToggleButtonStyle(isOn: playback.shuffle, size: 40, glyph: 16))
                .accessibilityLabel(Text("Shuffle"))
                .accessibilityValue(playback.shuffle ? Text("On") : Text("Off"))
                .help(Text("Shuffle"))

                Button {
                    playback.previous()
                } label: {
                    Image(systemName: "backward.end")
                }
                .buttonStyle(IconButtonStyle(size: 44, glyph: 20, prominent: true))
                .accessibilityLabel(Text("Previous"))
                .help(Text("Previous"))

                PlayPauseButton(size: 54, glyph: 22, prominent: true)

                Button {
                    playback.next()
                } label: {
                    Image(systemName: "forward.end")
                }
                .buttonStyle(IconButtonStyle(size: 44, glyph: 20, prominent: true))
                .accessibilityLabel(Text("Next"))
                .help(Text("Next"))

                Button {
                    playback.cycleRepeat()
                } label: {
                    Image(systemName: playback.repeatMode == .one ? "repeat.1" : "repeat")
                }
                .buttonStyle(ToggleButtonStyle(isOn: playback.repeatMode != .off, size: 40, glyph: 16))
                .accessibilityLabel(Text("Repeat"))
                .accessibilityValue(Text(repeatText))
                .help(Text("Repeat: \(repeatText)"))
            }

            // The scrubber thumb is the one Liquid Glass accent, in the Artwork style (deviation D2).
            SeekBar(liquidThumb: settings.current.style == .artwork)
                .frame(maxWidth: .infinity)

            if playback.showsVolume {
                VolumeSlider()
                    .frame(width: 168)
            }

            HStack(spacing: Tokens.Space.s1) {
                if playback.isRemote {
                    RemoteDevicesButton(size: 40, glyph: 17)
                } else {
                    OutputDeviceMenu()
                }
                NowPlayingMoreMenu()
            }
        }
        .padding(.leading, Tokens.Space.s5)
        .padding(.trailing, Tokens.Space.s4)
        .frame(height: LuminousGeometry.dockHeight)
    }

    private var repeatText: String {
        switch playback.repeatMode {
        case .off: return String(localized: "Off")
        case .all: return String(localized: "All")
        case .one: return String(localized: "One")
        }
    }
}

/// More, at the end of the dock: the song's menu without the queue's actions (it is playing), or
/// for a remote session Open in {Service} and Stop Controlling; Make Again and Delete Made Lyrics
/// for lyrics AI Lyrics made; then Desktop Lyrics.
struct NowPlayingMoreMenu: View {
    @Environment(PlaybackStore.self) private var playback
    @Environment(SettingsStore.self) private var settings
    @Environment(DesktopLyricsStore.self) private var desktopLyrics
    @Environment(AiLyricsStore.self) private var aiLyrics
    @Environment(\.palette) private var palette
    @State private var confirmDelete = false

    var body: some View {
        Menu {
            if playback.isRemote {
                RemoteMoreItems()
            } else if let now = playback.nowPlaying {
                TrackActionsMenu(items: [TrackItem(nowPlaying: now)], queueActions: false)
            }
            MadeLyricsMenuItems(confirmDelete: $confirmDelete)
            Divider()
            DesktopLyricsMenuItems(settings: settings, desktop: desktopLyrics)
        } label: {
            Image(systemName: "ellipsis")
                .font(.system(size: 17, weight: .semibold))
                .foregroundStyle(palette[Tokens.Ink.ink90])
                .frame(width: 40, height: 40)
                .contentShape(Circle())
        }
        .menuStyle(.button)
        .buttonStyle(.plain)
        .menuIndicator(.hidden)
        .fixedSize()
        .accessibilityLabel(Text("More Actions"))
        .help(Text("More Actions"))
        .confirmDeletingMadeLyrics($confirmDelete, aiLyrics: aiLyrics)
    }
}
