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
    /// Opens the item in its service (Open in {Service}).
    var link: String?
    /// The source it comes from, whose name names the service.
    var source: String?
    /// "Also in …": the other sources that have it, in unified search.
    var alsoIn: String? = nil

    var canPlay: Bool { playable && !missing && track != nil }

    /// Its main action (double-click, Return, VoiceOver's default action) opens its link: it
    /// cannot play here and has one.
    var opensLink: Bool { ServiceLink.isMainAction(canPlay: canPlay, link: link) }
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
                  playable: result.playable, unplayableReason: result.unplayableReason, link: result.link,
                  source: result.source == SearchStore.library ? nil : result.source)
    }

    /// The song the engine plays (Now Playing's More).
    init(nowPlaying now: NowPlayingInfo) {
        var source: String?
        if case .source(let song) = now.track { source = song.plugin }
        self.init(id: "now\(now.uid)", track: now.track, title: now.title, artist: now.artist, album: now.album,
                  durationMs: now.durationMs, artwork: now.artwork.flatMap { ArtworkSource(hash: nil, location: $0) },
                  path: now.path, link: now.link, source: source)
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
    @Environment(\.look) private var look
    @State private var hovering = false

    var body: some View {
        HStack(spacing: Tokens.Space.s3) {
            leading
            VStack(alignment: .leading, spacing: 1) {
                Text(item.title.isEmpty ? String(localized: "Unknown Title") : item.title)
                    .typography(Tokens.TypeRoles.listTitle)
                    .foregroundStyle(palette[isCurrent ? Tokens.Accent.accentText : Tokens.Ink.ink90])
                    .lineLimit(1)
                if let second = secondLine {
                    Text(second)
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
                if item.opensLink {
                    // Its main action opens it in its service.
                    Image(systemName: "arrow.up.right.square")
                        .foregroundStyle(palette[Tokens.Ink.ink50])
                        .accessibilityHidden(true)
                }
            }
            Text(item.durationMs.map { TimeText.from(milliseconds: $0) } ?? "")
                .typography(Tokens.TypeRoles.listSecondary)
                .monospacedDigit()
                .foregroundStyle(palette[Tokens.Ink.ink50])
                .frame(minWidth: 40, alignment: .trailing)
        }
        // The look's density, never shorter than the artwork with a little room.
        .frame(height: max(look.spacing(Tokens.Player.rowHeight), 44))
        .opacity(item.canPlay || item.track == nil ? 1 : 0.45)
        .background {
            RoundedRectangle(cornerRadius: look.radius(Tokens.Radius.rMd), style: .continuous)
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

    /// The artist, and the other sources that have the song ("Also in …").
    private var secondLine: String? {
        CountText.joined([item.artist, item.alsoIn]).nilIfEmpty
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
                        RoundedRectangle(cornerRadius: look.radius(Tokens.Artwork.radiusList), style: .continuous)
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
        if let alsoIn = item.alsoIn { parts.append(alsoIn) }
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

/// A song's menu (docs/ui.md): Play Next, Play Last, Add to Playlist…, Add to or Remove from
/// Favourites, Go to Album, Go to Artist, Create Synced Lyrics (library songs, while AI Lyrics is
/// offered), Open in {Service} and Show in Finder. A remote source's
/// songs never join the queue, playlists or favourites (ADR-0018), so they offer only what opens
/// elsewhere. Rows that do not carry a library song's album or file look them up when chosen.
struct TrackActionsMenu: View {
    /// The rows the actions apply to (the selection, or the row clicked).
    var items: [TrackItem]
    /// Play Next, Play Last and the favourites; off for the song playing now (Now Playing's More,
    /// beside its heart).
    var queueActions = true

    @Environment(PlaybackStore.self) private var playback
    @Environment(LibraryStore.self) private var library
    @Environment(SourcesStore.self) private var sources
    @Environment(Router.self) private var router
    @Environment(AiLyricsStore.self) private var aiLyrics
    @Environment(ProStore.self) private var pro

    /// The rows' tracks that can join the queue and playlists: playable, not a remote source's.
    private var tracks: [TrackReference] {
        items.filter { $0.canPlay && !sources.isRemote($0.track) }.compactMap(\.track)
    }

    var body: some View {
        if !tracks.isEmpty {
            if queueActions {
                Button("Play Next") {
                    playback.enqueue(tracks, next: true)
                }
                Button("Play Last") {
                    playback.enqueue(tracks, next: false)
                }
                Divider()
            }
            AddToPlaylistMenu(collection: .tracks(tracks))
            if queueActions {
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
        }
        if items.count == 1, let item = items.first {
            let link = ServiceLink.url(item.link) != nil && item.source != nil
            if !tracks.isEmpty && (Self.hasAlbum(item) || Self.hasArtist(item) || Self.hasFile(item) || link) {
                Divider()
            }
            if Self.hasAlbum(item) {
                Button("Go to Album") { goToAlbum(item) }
            }
            if Self.hasArtist(item) {
                Button("Go to Artist") { goToArtist(item) }
            }
            if aiLyrics.isOffered, let id = Self.songID(item) {
                Button("Create Synced Lyrics") {
                    aiLyrics.make(.library(id: id), pro: pro)
                }
            }
            OpenInServiceButton(link: item.link, source: item.source)
            if Self.hasFile(item) {
                Button("Show in Finder") { showInFinder(item) }
            }
        }
    }

    /// The library id of a library song.
    private static func songID(_ item: TrackItem) -> Int64? {
        if case .library(let id)? = item.track { return id }
        return nil
    }

    /// A file played without the library.
    private static func filePath(_ item: TrackItem) -> String? {
        if case .file(let path)? = item.track { return path }
        return nil
    }

    private static func hasAlbum(_ item: TrackItem) -> Bool {
        item.albumID != nil || (songID(item) != nil && !(item.album ?? "").isEmpty)
    }

    private static func hasArtist(_ item: TrackItem) -> Bool {
        songID(item) != nil && !(item.artist ?? "").isEmpty
    }

    private static func hasFile(_ item: TrackItem) -> Bool {
        item.path != nil || filePath(item) != nil || songID(item) != nil
    }

    private func goToAlbum(_ item: TrackItem) {
        router.showNowPlaying(false, reduceMotion: true)
        if let albumID = item.albumID {
            router.open(.album(AlbumSummary(id: albumID, title: item.album ?? "", albumArtist: item.artist,
                                            year: nil, trackCount: 0, artworkHash: nil)))
            return
        }
        guard let id = Self.songID(item) else { return }
        Task { @MainActor in
            guard let song = await library.song(id: id), let albumID = song.albumID else { return }
            router.open(.album(AlbumSummary(id: albumID, title: song.album ?? "",
                                            albumArtist: song.albumArtist ?? song.artist, year: song.year,
                                            trackCount: 0, artworkHash: song.artworkHash)))
        }
    }

    private func goToArtist(_ item: TrackItem) {
        guard let name = item.artist else { return }
        router.showNowPlaying(false, reduceMotion: true)
        Task { @MainActor in
            if let artist = await library.artist(named: name) {
                router.open(.artist(artist))
            }
        }
    }

    private func showInFinder(_ item: TrackItem) {
        if let path = item.path ?? Self.filePath(item) {
            Self.reveal(path)
            return
        }
        guard let id = Self.songID(item) else { return }
        Task { @MainActor in
            if let song = await library.song(id: id) {
                Self.reveal(song.path)
            }
        }
    }

    private static func reveal(_ path: String) {
        NSWorkspace.shared.activateFileViewerSelecting([URL(fileURLWithPath: path)])
    }
}

/// Add to Playlist…: a new playlist (named first) or one of the person's, for the songs chosen or a
/// whole album, artist or playlist. `excluding` leaves out the playlist whose page it is on.
struct AddToPlaylistMenu: View {
    var collection: CollectionReference
    var excluding: Int64?
    @Environment(LibraryStore.self) private var library

    var body: some View {
        Menu("Add to Playlist…") {
            Button("New Playlist…") {
                library.askForNewPlaylist(with: collection)
            }
            if !targets.isEmpty {
                Divider()
                ForEach(targets) { playlist in
                    Button(playlist.name) {
                        library.add(collection, toPlaylist: playlist.id)
                    }
                }
            }
        }
    }

    private var targets: [PlaylistInfo] {
        library.playlists.filter { $0.id != excluding }
    }
}

/// The More menu of an album, artist or playlist page (docs/ui.md): Play Next, Play Last and Add
/// to Playlist…, then what the page adds (a playlist's Rename…, Export Playlist… and Delete
/// Playlist…).
struct CollectionMoreMenu<Extra: View>: View {
    var collection: CollectionReference
    /// The playlist whose page this is, left out of Add to Playlist….
    var playlistID: Int64?
    @ViewBuilder var extra: Extra
    @Environment(PlaybackStore.self) private var playback

    var body: some View {
        Menu {
            Button("Play Next") {
                playback.enqueue(collection, next: true)
            }
            Button("Play Last") {
                playback.enqueue(collection, next: false)
            }
            AddToPlaylistMenu(collection: collection, excluding: playlistID)
            extra
        } label: {
            Label("More", systemImage: "ellipsis")
        }
        .menuStyle(.button)
        .buttonStyle(.bordered)
        .controlSize(.large)
        .fixedSize()
        .accessibilityLabel(Text("More Actions"))
    }
}

extension CollectionMoreMenu where Extra == EmptyView {
    init(collection: CollectionReference) {
        self.init(collection: collection, playlistID: nil) { EmptyView() }
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
                // A large title (Luminous: `type.text-h1`, 48 pt bold) that scrolls with the page.
                Text(title)
                    .lookFont(size: Tokens.TypeScale.textH1.rounded(), weight: .bold)
                    .lineLimit(2)
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

/// Play (the page's one accent fill) and Shuffle (glass), as a page's actions: for a collection
/// the core plays, or for a list it has no collection for (Songs), which `play` plays.
struct PlayShuffleButtons: View {
    private let collection: CollectionReference?
    private let action: ((_ shuffle: Bool) -> Void)?
    private let disabled: Bool
    @Environment(PlaybackStore.self) private var playback

    init(collection: CollectionReference, disabled: Bool = false) {
        self.collection = collection
        self.action = nil
        self.disabled = disabled
    }

    init(disabled: Bool = false, play: @escaping (_ shuffle: Bool) -> Void) {
        self.collection = nil
        self.action = play
        self.disabled = disabled
    }

    var body: some View {
        HStack(spacing: Tokens.Space.s2) {
            Button {
                play(shuffle: false)
            } label: {
                Label("Play", systemImage: "play.fill")
            }
            .buttonStyle(PillButtonStyle(prominent: true))
            Button {
                play(shuffle: true)
            } label: {
                Label("Shuffle", systemImage: "shuffle")
            }
            .buttonStyle(PillButtonStyle(prominent: false))
        }
        .disabled(disabled)
    }

    private func play(shuffle: Bool) {
        if let action {
            action(shuffle)
        } else if let collection {
            playback.play(collection, shuffle: shuffle)
        }
    }
}

/// A capsule button with a glyph and a label: accent-filled, or on a Mist surface.
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
            .lookFont(size: 13, weight: .semibold)
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
                    .surface(in: shape)
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
