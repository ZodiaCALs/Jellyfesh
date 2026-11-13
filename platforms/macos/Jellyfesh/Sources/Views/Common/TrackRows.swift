// Track rows for every list the core gives (library tracks, playlist and favourite rows, search
// and browse results), the actions on them, and the playing indicator. Rows are 52 pt
// (`player.row-height`) with the list type roles; hovering fills a row, the playing row is
// accent-soft with accent text and a level glyph, and on album pages the track number turns into a
// play button on hover (v0.0.4, after Apple Music and Media Player).

import AppKit
import SwiftUI

/// What a track row shows.
struct TrackItem: Identifiable, Hashable {
    /// Unique within its list.
    var id: String
    var track: TrackReference?
    var title: String
    var artist: String?
    var album: String?
    var durationMs: UInt64?
    var artwork: ArtworkSource?
    /// The track number, on album pages.
    var number: UInt32?
    /// Nil when not known.
    var favorite: Bool?
    /// The file is gone; the row cannot play.
    var missing = false
    var playable = true
    var unplayableReason: String?
    var albumID: Int64?
    /// A library file, for Show in Finder.
    var path: String?

    var canPlay: Bool { playable && !missing && track != nil }
}

extension TrackItem {
    init(song: Song, favorite: Bool? = nil, numbered: Bool = false) {
        self.init(id: "t\(song.id)", track: song.reference, title: song.title, artist: song.artist,
                  album: song.album, durationMs: song.durationMs, artwork: song.artworkHash.map(ArtworkSource.hash),
                  number: numbered ? song.trackNumber : nil, favorite: favorite, albumID: song.albumID,
                  path: song.path)
    }

    init(row: SongRow, id: String) {
        self.init(id: id, track: row.track, title: row.title, artist: row.artist, album: row.album,
                  durationMs: row.durationMs, artwork: ArtworkSource(hash: row.artworkHash, location: row.artwork),
                  favorite: row.favorite, missing: row.missing)
    }

    init(result: SourceResult, artwork: ArtworkSource?) {
        self.init(id: result.id, track: result.track, title: result.title, artist: result.subtitle,
                  album: result.album, durationMs: result.durationMs, artwork: artwork,
                  playable: result.playable, unplayableReason: result.unplayableReason)
    }
}

enum TrackRowStyle {
    /// Artwork, title and artist, album, duration.
    case full
    /// A track number instead of artwork, no album (album pages).
    case album
    /// Artwork, title and artist, duration (narrow places).
    case compact
}

struct TrackRowView: View {
    var item: TrackItem
    var style: TrackRowStyle = .full
    var isCurrent = false
    var isPlaying = false
    /// Plays from this row: on album pages its number turns into a play button on hover.
    var play: (() -> Void)?

    @Environment(\.palette) private var palette
    @Environment(\.effects) private var effects
    @State private var hovering = false

    var body: some View {
        HStack(spacing: Tokens.Space.s3) {
            leading
            VStack(alignment: .leading, spacing: 1) {
                Text(item.title.isEmpty ? String(localized: "Unknown Title") : item.title)
                    .typography(Tokens.TypeRoles.listTitle)
                    .foregroundStyle(palette[isCurrent ? Tokens.Accent.accentText : Tokens.Ink.ink90])
                    .lineLimit(1)
                if let artist = item.artist, !artist.isEmpty {
                    Text(artist)
                        .typography(Tokens.TypeRoles.listSecondary)
                        .foregroundStyle(palette[Tokens.Ink.ink70])
                        .lineLimit(1)
                }
            }
            .frame(maxWidth: .infinity, alignment: .leading)
            if style == .full, let album = item.album, !album.isEmpty {
                Text(album)
                    .typography(Tokens.TypeRoles.listSecondary)
                    .foregroundStyle(palette[Tokens.Ink.ink70])
                    .lineLimit(1)
                    .frame(maxWidth: .infinity, alignment: .leading)
            }
            if item.favorite == true {
                Image(systemName: "heart.fill")
                    .font(.system(size: 11))
                    .foregroundStyle(palette[Tokens.Accent.accentText])
                    .accessibilityHidden(true)
            }
            if let reason = unplayableNote {
                Image(systemName: "exclamationmark.circle")
                    .foregroundStyle(palette[Tokens.Ink.ink50])
                    .help(reason)
            }
            Text(item.durationMs.map { TimeText.from(milliseconds: $0) } ?? "")
                .typography(Tokens.TypeRoles.listSecondary)
                .monospacedDigit()
                .foregroundStyle(palette[Tokens.Ink.ink50])
                .frame(minWidth: 40, alignment: .trailing)
        }
        .frame(height: Tokens.Player.rowHeight)
        .opacity(item.canPlay || item.track == nil ? 1 : 0.45)
        .background {
            RoundedRectangle(cornerRadius: Tokens.Radius.rMd, style: .continuous)
                .fill(rowFill)
                .padding(.horizontal, -Tokens.Space.s2)
        }
        .contentShape(Rectangle())
        .onHover { hovering = $0 }
        .animation(Motion.unless(!effects.animates, Motion.hover), value: hovering)
        .accessibilityElement(children: .ignore)
        .accessibilityLabel(Text(accessibilityText))
        .accessibilityAddTraits(isCurrent ? .isSelected : [])
    }

    /// Hover `fill`; the playing row `accent-soft`.
    private var rowFill: Color {
        if isCurrent { return palette[Tokens.Accent.accentSoft] }
        return hovering ? palette[Tokens.Line.fill] : .clear
    }

    @ViewBuilder private var leading: some View {
        if style == .album {
            ZStack {
                if hovering, let play, item.canPlay {
                    Button(action: play) {
                        Image(systemName: "play.fill")
                            .font(.system(size: 11, weight: .semibold))
                            .foregroundStyle(palette[isCurrent ? Tokens.Accent.accentText : Tokens.Ink.ink90])
                            .frame(width: 28, height: 28)
                            .contentShape(Rectangle())
                    }
                    .buttonStyle(.plain)
                    .help(Text("Play"))
                } else if isCurrent {
                    LevelGlyph(playing: isPlaying)
                } else {
                    Text(item.number.map(String.init) ?? "")
                        .typography(Tokens.TypeRoles.listSecondary)
                        .monospacedDigit()
                        .foregroundStyle(palette[Tokens.Ink.ink50])
                }
            }
            .frame(width: 28)
        } else {
            ArtworkView(source: item.artwork, size: 40, cornerRadius: Tokens.Artwork.radiusList)
                .overlay {
                    if isCurrent {
                        RoundedRectangle(cornerRadius: Tokens.Artwork.radiusList, style: .continuous)
                            .fill(.black.opacity(0.35))
                        LevelGlyph(playing: isPlaying, color: .white)
                    }
                }
        }
    }

    private var unplayableNote: String? {
        if item.missing { return String(localized: "The file is missing.") }
        if !item.playable { return item.unplayableReason ?? String(localized: "This track cannot be played.") }
        return nil
    }

    private var accessibilityText: String {
        var parts = [item.title]
        if let artist = item.artist, !artist.isEmpty { parts.append(artist) }
        if let album = item.album, !album.isEmpty, style == .full { parts.append(album) }
        if let duration = item.durationMs { parts.append(TimeText.spoken(microseconds: Int64(clamping: duration) * 1_000)) }
        if isCurrent { parts.append(isPlaying ? String(localized: "Playing") : String(localized: "Paused")) }
        if item.favorite == true { parts.append(String(localized: "Favourite")) }
        if let note = unplayableNote { parts.append(note) }
        return parts.joined(separator: ", ")
    }
}

/// The playing row's mark: three level bars that move only while playing, and stand still while
/// paused, under reduced motion and while the window is hidden.
struct LevelGlyph: View {
    var playing: Bool
    var color: Color?
    @Environment(\.palette) private var palette
    @Environment(\.effects) private var effects

    /// Bar heights (fractions of the glyph) at rest and in the four phases they cycle through.
    private static let still: [CGFloat] = [0.55, 1, 0.4]
    private static let phases: [[CGFloat]] = [[0.35, 0.9, 0.55], [0.9, 0.45, 0.75], [0.5, 0.7, 0.3], [0.7, 0.3, 1]]

    var body: some View {
        let tint = color ?? palette[Tokens.Accent.accentText]
        Group {
            if playing && effects.animates && effects.windowVisible {
                PhaseAnimator([0, 1, 2, 3]) { phase in
                    LevelBars(levels: Self.phases[phase], tint: tint)
                } animation: { _ in
                    .easeInOut(duration: 0.3)
                }
            } else {
                LevelBars(levels: Self.still, tint: tint)
            }
        }
        .frame(width: 14, height: 12)
        .accessibilityHidden(true)
    }
}

private struct LevelBars: View {
    var levels: [CGFloat]
    var tint: Color

    var body: some View {
        HStack(alignment: .bottom, spacing: 2) {
            ForEach(levels.indices, id: \.self) { index in
                Capsule()
                    .fill(tint)
                    .frame(width: 3, height: max(3, 12 * levels[index]))
            }
        }
        .frame(height: 12, alignment: .bottom)
    }
}

/// Play Next, Play Later, Add to Playlist, Favourite, Go to Album, Show in Finder.
struct TrackActionsMenu: View {
    /// The rows the actions apply to (the selection, or the row clicked).
    var items: [TrackItem]

    @Environment(PlaybackStore.self) private var playback
    @Environment(LibraryStore.self) private var library
    @Environment(Router.self) private var router

    private var tracks: [TrackReference] {
        items.filter(\.canPlay).compactMap(\.track)
    }

    var body: some View {
        if !tracks.isEmpty {
            Button("Play") {
                playback.play(tracks: tracks)
            }
            Button("Play Next") {
                playback.enqueue(tracks, next: true)
            }
            Button("Play Later") {
                playback.enqueue(tracks, next: false)
            }
            Divider()
            AddToPlaylistMenu(tracks: tracks)
            if items.count == 1, let item = items.first, let track = item.track {
                if item.favorite == true {
                    Button("Remove from Favourites") { library.setFavorite(track, false) }
                } else {
                    Button("Add to Favourites") { library.setFavorite(track, true) }
                }
            } else {
                Button("Add to Favourites") {
                    for track in tracks { library.setFavorite(track, true) }
                }
            }
        }
        if items.count == 1, let item = items.first {
            if let albumID = item.albumID {
                Divider()
                Button("Go to Album") {
                    router.showNowPlaying(false, reduceMotion: true)
                    router.open(.album(AlbumSummary(id: albumID, title: item.album ?? "", albumArtist: item.artist,
                                                    year: nil, trackCount: 0, artworkHash: nil)))
                }
            }
            if let path = item.path {
                Button("Show in Finder") {
                    NSWorkspace.shared.activateFileViewerSelecting([URL(fileURLWithPath: path)])
                }
            }
        }
    }
}

/// The playlists to add tracks to, and a new one.
struct AddToPlaylistMenu: View {
    var tracks: [TrackReference]
    @Environment(LibraryStore.self) private var library

    var body: some View {
        Menu("Add to Playlist") {
            Button("New Playlist") {
                library.createPlaylist(name: String(localized: "New Playlist"), tracks: tracks)
            }
            if !library.playlists.isEmpty {
                Divider()
                ForEach(library.playlists) { playlist in
                    Button(playlist.name) {
                        library.add(tracks, toPlaylist: playlist.id)
                    }
                }
            }
        }
    }
}

/// A section's header on library pages: a title, an optional line under it and actions.
struct PageHeader<Actions: View>: View {
    var title: String
    var subtitle: String?
    @ViewBuilder var actions: Actions
    @Environment(\.palette) private var palette

    var body: some View {
        HStack(alignment: .lastTextBaseline, spacing: Tokens.Space.s4) {
            VStack(alignment: .leading, spacing: Tokens.Space.s1) {
                // A large title (`type.text-h2`, bold) that scrolls with the page (after Apple Music).
                Text(title)
                    .font(.system(size: Tokens.TypeScale.textH2.rounded(), weight: .bold))
                    .foregroundStyle(palette[Tokens.Ink.ink90])
                    .accessibilityAddTraits(.isHeader)
                if let subtitle, !subtitle.isEmpty {
                    Text(subtitle)
                        .typography(Tokens.TypeRoles.listSecondary)
                        .foregroundStyle(palette[Tokens.Ink.ink70])
                }
            }
            Spacer(minLength: 0)
            actions
        }
    }
}

extension PageHeader where Actions == EmptyView {
    init(title: String, subtitle: String? = nil) {
        self.init(title: title, subtitle: subtitle) { EmptyView() }
    }
}

/// Play (the page's one accent fill) and Shuffle (glass) for a collection, as a page's actions.
struct PlayShuffleButtons: View {
    var collection: CollectionReference
    var disabled = false
    @Environment(PlaybackStore.self) private var playback

    var body: some View {
        HStack(spacing: Tokens.Space.s2) {
            Button {
                playback.play(collection)
            } label: {
                Label("Play", systemImage: "play.fill")
            }
            .buttonStyle(PillButtonStyle(prominent: true))
            Button {
                playback.play(collection, shuffle: true)
            } label: {
                Label("Shuffle", systemImage: "shuffle")
            }
            .buttonStyle(PillButtonStyle(prominent: false))
        }
        .disabled(disabled)
    }
}

/// A capsule button with a glyph and a label: accent-filled, or on Mist glass.
struct PillButtonStyle: ButtonStyle {
    var prominent: Bool

    func makeBody(configuration: Configuration) -> some View {
        PillButtonBody(configuration: configuration, prominent: prominent)
    }
}

private struct PillButtonBody: View {
    let configuration: ButtonStyleConfiguration
    let prominent: Bool
    @State private var hovering = false
    @Environment(\.palette) private var palette
    @Environment(\.effects) private var effects
    @Environment(\.isEnabled) private var isEnabled

    var body: some View {
        let shape = Capsule()
        let label = configuration.label
            .font(.system(size: 13, weight: .semibold))
            .padding(.horizontal, 18)
            .frame(minWidth: 96, minHeight: 36)
            .contentShape(shape)
        Group {
            if prominent {
                label
                    .foregroundStyle(palette[Tokens.Accent.onAccent])
                    .background(shape.fill(palette[configuration.isPressed
                        ? Tokens.Accent.accentPress
                        : (hovering ? Tokens.Accent.accentHover : Tokens.Accent.accent)]))
            } else {
                label
                    .foregroundStyle(palette[Tokens.Ink.ink90])
                    .glass(.mist, in: shape)
                    .overlay(shape.fill(palette[Tokens.Ink.ink06]).opacity(hovering ? 1 : 0))
            }
        }
        .scaleEffect(configuration.isPressed ? 0.96 : 1)
        .opacity(isEnabled ? 1 : 0.4)
        .animation(Motion.unless(!effects.animates, Motion.press), value: configuration.isPressed)
        .animation(Motion.unless(!effects.animates, Motion.hover), value: hovering)
        .onHover { hovering = $0 }
    }
}

extension View {
    /// List rows without separators or default insets, over the Misty field.
    func trackListRow() -> some View {
        listRowInsets(EdgeInsets(top: 0, leading: Tokens.Space.s4, bottom: 0, trailing: Tokens.Space.s4))
            .listRowSeparator(.hidden)
            .listRowBackground(Color.clear)
    }

    /// A list over the Misty field.
    func fieldList() -> some View {
        listStyle(.plain)
            .scrollContentBackground(.hidden)
    }
}
