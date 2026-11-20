// Sources (docs/ui.md): what sources are, Add a Source (the core's services, connected as in
// Settings, and Apple Music where this copy of the app can use it), Your Sources (the installed
// plugins with their badges and accounts, and Apple Music once on; choosing one opens its Discover
// page, which browses its catalogue page by page from Browse) and Sources Settings…, where
// installing, enabling and each source's settings live.

import SwiftUI

struct SourcesView: View {
    @Environment(AppModel.self) private var model
    @Environment(SourcesStore.self) private var sources
    @Environment(AppleMusicStore.self) private var appleMusic
    @Environment(\.openSettings) private var openSettings
    @Environment(\.palette) private var palette
    /// The service whose Connect sheet is open.
    @State private var connecting: SourcePreset?

    var body: some View {
        List {
            VStack(alignment: .leading, spacing: Tokens.Space.s2) {
                PageHeader(title: String(localized: "Sources"), subtitle: nil)
                Text("Sources bring music from servers and services into Jellyfesh.")
                    .typography(Tokens.TypeRoles.listSecondary)
                    .foregroundStyle(palette[Tokens.Ink.ink70])
                    .fixedSize(horizontal: false, vertical: true)
            }
            .padding(.vertical, Tokens.Space.s4)
            .trackListRow()
            if sources.unavailable {
                SourcesUnavailableNotice()
                    .padding(.bottom, Tokens.Space.s3)
                    .trackListRow()
            }
            if !sources.presets.isEmpty || offersAppleMusic {
                SectionTitle("Add a Source")
                    .padding(.top, Tokens.Space.s3)
                    .trackListRow()
                ForEach(sources.presets) { preset in
                    PresetRow(preset: preset, plugin: sources.plugin(preset.plugin), iconSize: 40) {
                        connecting = preset
                    }
                    .typography(Tokens.TypeRoles.listTitle)
                    .frame(minHeight: Tokens.Player.rowHeight)
                    .trackListRow()
                }
                if offersAppleMusic {
                    AppleMusicOfferRow()
                        .trackListRow()
                }
            }
            SectionTitle("Your Sources")
                .padding(.top, Tokens.Space.s5)
                .trackListRow()
            if appleMusic.isAvailable {
                NavigationLink(value: Route.appleMusic) {
                    AppleMusicSourceRow()
                }
                .trackListRow()
            }
            if sources.plugins.isEmpty && !appleMusic.isAvailable {
                VStack(alignment: .leading, spacing: 2) {
                    Text("No Sources Installed")
                        .typography(Tokens.TypeRoles.listTitle)
                        .foregroundStyle(palette[Tokens.Ink.ink90])
                    Text("Install a source plugin from a .jfpkg file.")
                        .typography(Tokens.TypeRoles.listSecondary)
                        .foregroundStyle(palette[Tokens.Ink.ink70])
                }
                .padding(.vertical, Tokens.Space.s2)
                .accessibilityElement(children: .combine)
                .trackListRow()
            }
            ForEach(sources.plugins) { plugin in
                if plugin.enabled && (plugin.canBrowse || plugin.hasDiscover) {
                    NavigationLink(value: Route.discover(plugin: plugin.id, title: plugin.name)) {
                        SourceRow(plugin: plugin)
                    }
                    .trackListRow()
                } else {
                    SourceRow(plugin: plugin)
                        .trackListRow()
                }
            }
            HStack {
                Button("Sources Settings…") {
                    model.showSettings(.sources, using: openSettings)
                }
                .buttonStyle(.bordered)
                Spacer(minLength: 0)
            }
            .padding(.vertical, Tokens.Space.s5)
            .trackListRow()
        }
        .fieldList()
        .navigationTitle(Text("Sources"))
        .connectSourceFlow($connecting, in: .main)
    }

    /// Apple Music can be turned on here (or is being checked).
    private var offersAppleMusic: Bool {
        appleMusic.availability == .offered || appleMusic.availability == .checking
    }
}

/// A plugin with its badge line and account.
struct SourceRow: View {
    var plugin: SourcePlugin
    @Environment(\.palette) private var palette

    var body: some View {
        HStack(spacing: Tokens.Space.s3) {
            PluginIcon(plugin: plugin, size: 40)
            VStack(alignment: .leading, spacing: 1) {
                Text(plugin.name)
                    .typography(Tokens.TypeRoles.listTitle)
                    .foregroundStyle(palette[Tokens.Ink.ink90])
                Text(CountText.joined([SourceText.badge(plugin), SourceText.account(plugin)]))
                    .typography(Tokens.TypeRoles.listSecondary)
                    .foregroundStyle(palette[Tokens.Ink.ink70])
            }
            Spacer(minLength: 0)
            if !plugin.enabled {
                Text("Off")
                    .lookFont(size: 12, weight: .semibold)
                    .foregroundStyle(palette[Tokens.Ink.ink50])
            }
        }
        .frame(height: Tokens.Player.rowHeight)
        .accessibilityElement(children: .combine)
    }
}

/// The plugin's own icon when it has one, else a generic glyph (also when there is no plugin, for
/// a service in Add a Source whose plugin is not listed).
struct PluginIcon: View {
    var plugin: SourcePlugin?
    var size: CGFloat

    var body: some View {
        if let path = plugin?.iconPath {
            ArtworkView(source: .file(path), size: size, cornerRadius: Tokens.Radius.rMd,
                        placeholder: "antenna.radiowaves.left.and.right")
        } else {
            ArtworkView(source: nil, size: size, cornerRadius: Tokens.Radius.rMd,
                        placeholder: "antenna.radiowaves.left.and.right")
        }
    }
}

/// The badge and account lines of a source (ADR-0004, ADR-0012), and of a service to add.
enum SourceText {
    static func badge(_ plugin: SourcePlugin) -> String {
        var parts = [plugin.badge == .official ? String(localized: "Official") : String(localized: "Community · unofficial")]
        if plugin.vendorPlayback {
            parts.append(String(localized: "Vendor playback"))
        }
        return parts.joined(separator: " · ")
    }

    static func account(_ plugin: SourcePlugin) -> String? {
        if let account = plugin.account {
            return String(localized: "Signed in as \(account)")
        }
        return plugin.signInMethods.isEmpty ? nil : String(localized: "Not signed in")
    }

    /// The line under a service in Add a Source: its account once signed in, else what it is
    /// (by the service's id; one without a description has no line).
    static func detail(_ preset: SourcePreset) -> String? {
        if let account = preset.account {
            return String(localized: "Signed in as \(account)")
        }
        switch preset.id {
        case "jellyfin": return String(localized: "Your Jellyfin server")
        case "navidrome": return String(localized: "Your Navidrome server")
        case "subsonic": return String(localized: "Airsonic, Gonic and other Subsonic servers")
        case "qqmusic": return String(localized: "Your QQ Music account")
        case "spotify": return String(localized: "Your Spotify devices")
        default: return nil
        }
    }
}

/// Said once a source command answered that sources are not in this build of the core yet.
struct SourcesUnavailableNotice: View {
    @Environment(\.palette) private var palette

    var body: some View {
        HStack(alignment: .top, spacing: Tokens.Space.s3) {
            Image(systemName: "clock.badge.checkmark")
                .font(.system(size: 18))
                .foregroundStyle(palette[Tokens.Semantic.info])
            VStack(alignment: .leading, spacing: Tokens.Space.s1) {
                Text("Sources Are on Their Way")
                    .lookFont(size: 13, weight: .semibold)
                    .foregroundStyle(palette[Tokens.Ink.ink90])
                Text("This build of the player cannot install, sign in to or browse sources yet. They arrive in an update.")
                    .lookFont(size: 12)
                    .foregroundStyle(palette[Tokens.Ink.ink70])
                    .fixedSize(horizontal: false, vertical: true)
            }
        }
        .padding(Tokens.Space.s4)
        .frame(maxWidth: .infinity, alignment: .leading)
        .surfaceCard()
        .accessibilityElement(children: .combine)
    }
}

/// One container of a source's catalogue (`node` nil is its root), page by page, as the web's
/// source pages show it (docs/parity.md N18). An album, playlist or artist has the header of the
/// library's pages (CollectionHeader): its artwork, kind, title and artist, the song count once
/// every page is in, Play and Shuffle (the core plays the whole container), and More (Play Next,
/// Play Last and Add to Playlist… where its songs may join the queue, then Open in {Service}). An
/// album's songs are numbered, and the songs end with "12 songs, 48 minutes". An album's or
/// artist's page from a source that describes its items (ADR-0022) adds the source's picture and
/// its notes or biography, which More expands. The source's top level and its folders have their
/// title and the source's line. A source whose terms ask for it is credited ("Content from
/// {Service}").
struct BrowseView: View {
    var plugin: String
    var node: String?
    var title: String
    var link: String?
    /// The container as the list it was chosen from showed it, when known.
    var item: SourceResult?

    @Environment(SourcesStore.self) private var sources
    @Environment(PlaybackStore.self) private var playback
    @Environment(AppModel.self) private var app
    @Environment(\.palette) private var palette
    @State private var model: BrowseModel?
    /// What the source says about the album or artist.
    @State private var details: DetailsAnswer?

    var body: some View {
        let items = model?.items ?? []
        let songs = model?.songs ?? []
        let numbers: [String: UInt32] = item?.kind == .album ? (model?.songNumbers() ?? [:]) : [:]
        List {
            header(songs: songs)
                .padding(.vertical, collectionKind == nil ? Tokens.Space.s4 : Tokens.Space.s5)
                .trackListRow()
            if let notes = details?.notes?.trimmingCharacters(in: .whitespacesAndNewlines), !notes.isEmpty {
                ExpandableNotes(text: notes)
                    .padding(.bottom, Tokens.Space.s4)
                    .trackListRow()
            }
            if let model {
                if model.unavailable {
                    SourcesUnavailableNotice()
                        .frame(maxWidth: 520)
                        .padding(.vertical, Tokens.Space.s4)
                        .trackListRow()
                } else if let error = model.error, items.isEmpty {
                    EmptyState(title: "This Source Did Not Answer", symbol: "exclamationmark.triangle", error: error)
                        .frame(minHeight: 240)
                        .trackListRow()
                } else if model.isLoading && items.isEmpty {
                    HStack {
                        Spacer()
                        ProgressView()
                        Spacer()
                    }
                    .padding(.vertical, Tokens.Space.s6)
                    .trackListRow()
                } else if model.loaded && items.isEmpty {
                    EmptyState(title: "Nothing Here", symbol: "tray")
                        .frame(minHeight: 240)
                        .trackListRow()
                }
            }
            ForEach(items) { entry in
                row(for: entry, number: numbers[entry.id])
                    .trackListRow()
            }
            if let model, model.nextPage != nil {
                HStack {
                    Spacer()
                    Button("Load More") { model.loadMore(sources) }
                        .disabled(model.isLoading)
                    Spacer()
                }
                .trackListRow()
            }
            if let model, model.isComplete, !songs.isEmpty {
                Text(CountText.songsAndLength(count: songs.count,
                                              milliseconds: songs.reduce(0) { $0 + ($1.durationMs ?? 0) }))
                    .typography(Tokens.TypeRoles.listSecondary)
                    .foregroundStyle(palette[Tokens.Ink.ink50])
                    .padding(.top, Tokens.Space.s4)
                    .padding(.bottom, Tokens.Space.s6)
                    .trackListRow()
            }
        }
        .fieldList()
        .navigationTitle(Text(title))
        .task(id: "\(plugin)\u{1F}\(node ?? "")") {
            let browse = BrowseModel(plugin: plugin, node: node)
            model = browse
            browse.start(sources)
        }
        .task(id: item?.id) {
            details = nil
            guard let item, item.kind == .album || item.kind == .artist,
                  sources.plugin(plugin)?.hasDetails == true, let reference = item.itemReference
            else { return }
            app.answers.details(reference, onAnswer: { answer in
                if answer.error == nil { details = answer }
            }, onFailure: { _ in })
        }
    }

    /// An album, playlist or artist (they get the collection header); nil for a folder or the
    /// source's top level.
    private var collectionKind: ResultKind? {
        guard let kind = item?.kind, kind == .album || kind == .playlist || kind == .artist else { return nil }
        return kind
    }

    @ViewBuilder private func header(songs: [SourceResult]) -> some View {
        if let item, let kind = collectionKind {
            CollectionHeader(
                artwork: (item.artwork ?? details?.image).flatMap { ArtworkSource(hash: nil, location: $0) },
                placeholder: SourcePage.placeholder(kind),
                round: kind == .artist,
                kind: SourcePage.kindName(kind),
                title: title,
                artist: kind == .artist ? nil : item.subtitle,
                facts: facts(songs: songs),
                credit: sources.attribution(of: plugin)
            ) {
                actions(for: item, songs: songs)
            }
        } else {
            PageHeader(title: title, subtitle: sourceLine) {
                if ServiceLink.url(link) != nil {
                    SourcePageMoreMenu(plugin: plugin, collection: nil, tracks: [], link: link)
                }
            }
        }
    }

    /// Play and Shuffle while the container plays here, else Open in {Service} as its main action;
    /// then More.
    @ViewBuilder private func actions(for item: SourceResult, songs: [SourceResult]) -> some View {
        let collection = CollectionReference.source(plugin: plugin, id: item.itemID)
        let playable = SourcePage.canPlay(item: item, songs: songs, complete: model?.isComplete ?? false)
        let tracks = model?.isComplete == true ? (model?.playableTracks ?? []) : []
        if playable {
            PlayShuffleButtons(collection: collection)
        } else if ServiceLink.url(link) != nil {
            Button {
                ServiceLink.open(link)
            } label: {
                Label(String(localized: "Open in \(sources.name(of: plugin))"), systemImage: "arrow.up.right.square")
            }
            .buttonStyle(PillButtonStyle(prominent: true))
        }
        if SourcePageMoreMenu.hasItems(remote: sources.isRemote(plugin), playable: playable, link: link) {
            SourcePageMoreMenu(plugin: plugin, collection: playable ? collection : nil, tracks: tracks, link: link)
        }
    }

    /// The source's name (unless the credit names it) and, once every page is in, the songs.
    private func facts(songs: [SourceResult]) -> String {
        CountText.joined([
            sources.attribution(of: plugin) == nil ? sources.name(of: plugin) : nil,
            model?.isComplete == true && !songs.isEmpty ? CountText.songs(songs.count) : nil,
        ])
    }

    /// Under a folder's or the top level's title: the credit, else the source's badge and account at
    /// its top level, its name in a folder.
    private var sourceLine: String? {
        if let credit = sources.attribution(of: plugin) { return credit }
        if node == nil, let source = sources.plugin(plugin) {
            return CountText.joined([SourceText.badge(source), SourceText.account(source)])
        }
        return sources.name(of: plugin)
    }

    @ViewBuilder private func row(for entry: SourceResult, number: UInt32?) -> some View {
        let artwork = entry.artwork.flatMap { ArtworkSource(hash: nil, location: $0) }
        if entry.kind == .track {
            let shown = SourcePage.trackItem(entry, artwork: artwork, number: number)
            let playFromRow: (() -> Void)? = number == nil ? nil : { play(from: entry) }
            TrackRowView(item: shown, style: number == nil ? .full : .album,
                         isCurrent: entry.track != nil && entry.track == playback.nowPlaying?.track,
                         isPlaying: playback.isPlaying, play: playFromRow)
                .onTapGesture(count: 2) { play(from: entry) }
                .contextMenu {
                    TrackActionsMenu(items: [TrackItem(result: entry, artwork: nil)])
                }
                .accessibilityAction { play(from: entry) }
        } else {
            // ResultRow's own menu: Play, Shuffle, Play Next, Play Last (not for a remote source)
            // and Open in {Service}.
            ResultRow(item: entry, artwork: artwork)
        }
    }

    /// Plays the page's tracks from `entry`; one that cannot play here opens its link instead.
    private func play(from entry: SourceResult) {
        guard let queue = model?.tracks(startingAt: entry) else {
            if ServiceLink.isMainAction(canPlay: entry.playable && entry.track != nil, link: entry.link) {
                ServiceLink.open(entry.link)
            }
            return
        }
        playback.play(tracks: queue.tracks, from: queue.start)
    }
}

/// What a source's page shows (BrowseView), as rules apart from the view.
enum SourcePage {
    /// Whether the container plays here: the list it came from said so, and once its songs are in,
    /// at least one of them can.
    static func canPlay(item: SourceResult, songs: [SourceResult], complete: Bool) -> Bool {
        guard item.playable else { return false }
        guard complete, !songs.isEmpty else { return true }
        return songs.contains { $0.playable && $0.track != nil }
    }

    static func kindName(_ kind: ResultKind) -> String {
        switch kind {
        case .album: return String(localized: "Album")
        case .playlist: return String(localized: "Playlist")
        case .artist: return String(localized: "Artist")
        case .track: return String(localized: "Song")
        case .folder: return String(localized: "Folder")
        }
    }

    static func placeholder(_ kind: ResultKind) -> String {
        switch kind {
        case .album: return "square.stack"
        case .playlist: return "music.note.list"
        case .artist: return "music.mic"
        case .track: return "music.note"
        case .folder: return "folder"
        }
    }

    /// A source song's album (Go to Album), for its page's header until the source says more.
    static func album(plugin: String, id: String, title: String?, artist: String?) -> SourceResult {
        SourceResult(source: plugin, kind: .album, itemID: id, title: title?.nilIfEmpty ?? String(localized: "Album"),
                     subtitle: artist, album: nil, durationMs: nil, artwork: nil, playable: true,
                     unplayableReason: nil, track: nil, alsoIn: [])
    }

    /// A source song's artist (Go to Artist).
    static func artist(plugin: String, id: String, name: String?) -> SourceResult {
        SourceResult(source: plugin, kind: .artist, itemID: id, title: name?.nilIfEmpty ?? String(localized: "Artist"),
                     subtitle: nil, album: nil, durationMs: nil, artwork: nil, playable: true,
                     unplayableReason: nil, track: nil, alsoIn: [])
    }

    /// A song of the page as its row shows it; numbered on an album.
    static func trackItem(_ entry: SourceResult, artwork: ArtworkSource?, number: UInt32?) -> TrackItem {
        var shown = TrackItem(result: entry, artwork: artwork)
        shown.number = number
        return shown
    }
}

/// More on a source's page: Play Next and Play Last for a container whose songs may join the queue
/// (not a remote source's), Add to Playlist… once every page of its songs is in, and Open in
/// {Service}.
private struct SourcePageMoreMenu: View {
    var plugin: String
    /// The container to queue; nil when it cannot play here.
    var collection: CollectionReference?
    /// Its playable songs, when every page is in.
    var tracks: [TrackReference]
    var link: String?

    @Environment(PlaybackStore.self) private var playback
    @Environment(SourcesStore.self) private var sources

    /// Whether the menu has anything to offer.
    static func hasItems(remote: Bool, playable: Bool, link: String?) -> Bool {
        (playable && !remote) || ServiceLink.url(link) != nil
    }

    var body: some View {
        Menu {
            if let collection, !sources.isRemote(plugin) {
                Button("Play Next") { playback.enqueue(collection, next: true) }
                Button("Play Last") { playback.enqueue(collection, next: false) }
                if !tracks.isEmpty {
                    AddToPlaylistMenu(collection: .tracks(tracks))
                }
                if ServiceLink.url(link) != nil {
                    Divider()
                }
            }
            OpenInServiceButton(link: link, source: plugin)
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

/// An album's notes or an artist's biography: three lines, and More shows the rest.
struct ExpandableNotes: View {
    var text: String
    @Environment(\.palette) private var palette
    @State private var expanded = false

    var body: some View {
        VStack(alignment: .leading, spacing: Tokens.Space.s1) {
            Text(text)
                .typography(Tokens.TypeRoles.listSecondary)
                .foregroundStyle(palette[Tokens.Ink.ink70])
                .lineLimit(expanded ? nil : 3)
                .fixedSize(horizontal: false, vertical: true)
                .textSelection(.enabled)
            if !expanded {
                Button("More") {
                    expanded = true
                }
                .buttonStyle(.link)
            }
        }
        .frame(maxWidth: 720, alignment: .leading)
    }
}
