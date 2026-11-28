// Pieces the library pages share: cards (albums, and the sources' items in SearchView), a track
// list that plays on double-click or Return, and the empty and loading states.

import SwiftUI

/// A card in a grid or shelf: the cover (`artwork.radius-grid`, Luminous 16 pt on `shadow-2`), the
/// title and a second line. Hovering or focusing lifts the cover 4 pt onto `shadow-3` and shows, on
/// it, a 40 pt play button in its bottom start corner (a card that plays) and More in its top end
/// corner; a press scales it to 97 %. Right-click opens the same menu as More (docs/ui.md, "Rows
/// and cards", v0.0.9).
struct CoverCard<Items: View>: View {
    var artwork: ArtworkSource?
    var placeholder = "square.stack"
    /// Artwork side in points; nil fills the grid cell.
    var size: CGFloat?
    var title: String
    var subtitle: String?
    /// What VoiceOver reads for the card.
    var accessibilityText: String
    /// The card's main action: opens its page, or plays it where it has none.
    var action: () -> Void
    /// The play button; nil for a card that does not play from its cover (an artist, a song).
    var play: (() -> Void)?
    /// The card's menu, for More and right-click.
    @ViewBuilder var menu: Items

    @Environment(\.palette) private var palette
    @Environment(\.effects) private var effects
    @State private var hovering = false
    /// The cover, the play button or More has keyboard focus: the card stays lifted.
    @FocusState private var focus: CardControl?

    var body: some View {
        let lifted = hovering || focus != nil
        VStack(alignment: .leading, spacing: Tokens.Space.s2) {
            Button(action: action) {
                ArtworkView(source: artwork, size: size, cornerRadius: Tokens.Artwork.radiusGrid,
                            placeholder: placeholder)
                    .elevation(lifted ? Tokens.Elevation.shadow3 : Tokens.Elevation.shadow2)
            }
            .buttonStyle(PressScaleStyle(scale: 0.97))
            .focused($focus, equals: .cover)
            .overlay(alignment: .bottomLeading) {
                if lifted, let play {
                    Button(action: play) {
                        Image(systemName: "play.fill")
                            .font(.system(size: 15, weight: .semibold))
                            .foregroundStyle(palette[Tokens.Ink.ink90])
                            .frame(width: 40, height: 40)
                            .surface(in: Circle())
                            .contentShape(Circle())
                    }
                    .buttonStyle(PressScaleStyle(scale: 0.92))
                    .focused($focus, equals: .play)
                    .padding(Tokens.Space.s2)
                    .accessibilityLabel(Text("Play \(title)"))
                    .help(Text("Play \(title)"))
                    .transition(.opacity)
                }
            }
            .overlay(alignment: .topTrailing) {
                if lifted {
                    MoreMenuButton(title: title, onCover: true) { menu }
                        .focused($focus, equals: .more)
                        .padding(Tokens.Space.s2)
                        .transition(.opacity)
                }
            }
            .offset(y: lifted && effects.animates ? -LuminousGeometry.coverLift : 0)
            .animation(Motion.unless(!effects.animates, Motion.hover), value: lifted)
            VStack(alignment: .leading, spacing: 1) {
                Text(title)
                    .typography(Tokens.TypeRoles.listTitle)
                    .foregroundStyle(palette[Tokens.Ink.ink90])
                    .lineLimit(1)
                if let subtitle, !subtitle.isEmpty {
                    Text(subtitle)
                        .typography(Tokens.TypeRoles.listSecondary)
                        .foregroundStyle(palette[Tokens.Ink.ink70])
                        .lineLimit(1)
                }
            }
        }
        .frame(width: size)
        .onHover { hovering = $0 }
        .contextMenu { menu }
        .accessibilityElement(children: .combine)
        .accessibilityLabel(Text(accessibilityText))
        .accessibilityAddTraits(.isButton)
        .accessibilityAction { action() }
        .modifier(CardPlayAction(title: title, play: play))
    }
}

/// A card's controls that keep it lifted while one has keyboard focus.
enum CardControl: Hashable {
    case cover, play, more
}

/// "Play {title}" among VoiceOver's actions on a card that plays, which shows its play button only
/// on hover or focus; VoiceOver's Show Menu opens the card's menu.
private struct CardPlayAction: ViewModifier {
    var title: String
    var play: (() -> Void)?

    @ViewBuilder func body(content: Content) -> some View {
        if let play {
            content.accessibilityAction(named: Text("Play \(title)"), play)
        } else {
            content
        }
    }
}

/// An album in a grid or shelf (CoverCard): title and artist (and in Search the other sources that
/// have it). Opens the album; its play button plays it.
struct AlbumCard: View {
    var album: AlbumSummary
    /// Artwork side in points; nil fills the grid cell.
    var size: CGFloat?
    /// "Also in …", in unified search.
    var alsoIn: String? = nil

    @Environment(Router.self) private var router
    @Environment(PlaybackStore.self) private var playback

    var body: some View {
        CoverCard(artwork: album.artworkHash.map(ArtworkSource.hash), size: size,
                  title: album.title.isEmpty ? String(localized: "Unknown Album") : album.title,
                  subtitle: CountText.joined([album.albumArtist ?? String(localized: "Unknown Artist"), alsoIn]),
                  accessibilityText: [album.title, album.albumArtist, alsoIn].compactMap { $0 }.joined(separator: ", "),
                  action: { router.open(.album(album)) },
                  play: { playback.play(.album(id: album.id)) }) {
            AlbumCardMenu(album: album)
        }
    }
}

/// A library album card's menu, its More and its right-click alike (docs/ui.md, "Rows and cards"):
/// Play and Shuffle, then the album page's More (Play Next, Play Last, Add to Playlist…) with Go to
/// Artist, then Get Info.
struct AlbumCardMenu: View {
    var album: AlbumSummary

    @Environment(PlaybackStore.self) private var playback
    @Environment(LibraryStore.self) private var library
    @Environment(Router.self) private var router

    var body: some View {
        Button("Play") { playback.play(.album(id: album.id)) }
        Button("Shuffle") { playback.play(.album(id: album.id), shuffle: true) }
        Divider()
        CollectionMenuItems(collection: .album(id: album.id)) {
            if let artist = album.albumArtist, !artist.isEmpty {
                Button("Go to Artist") { goToArtist(named: artist) }
            }
        }
    }

    /// The library's artist with this name, if it has one.
    private func goToArtist(named name: String) {
        Task { @MainActor in
            if let artist = await library.artist(named: name) {
                router.open(.artist(artist))
            }
        }
    }
}

/// The header of an album, playlist or artist page, the library's and a source's alike (docs/ui.md,
/// "Album, artist and playlist pages"): Back, the artwork (240 pt, `artwork.radius-grid` on
/// shadow-2; a circle for an artist), the kind, the title, the artist in accent text (a link when it
/// opens the artist), the facts and a credit, then the page's actions (Play, Shuffle, More).
struct CollectionHeader<Actions: View>: View {
    var artwork: ArtworkSource?
    var placeholder: String
    /// An artist's picture is round.
    var round = false
    /// "Album", "Playlist", "Artist".
    var kind: String? = nil
    var title: String
    var artist: String? = nil
    /// Opens the artist; nil shows the artist as text.
    var openArtist: (() -> Void)? = nil
    /// "Rock · 2019 · 12 songs".
    var facts: String? = nil
    /// "Content from {Service}", for a source that asks for it.
    var credit: String? = nil
    @ViewBuilder var actions: Actions

    @Environment(\.palette) private var palette
    @Environment(\.showsBackButton) private var showsBackButton

    static var artworkSide: CGFloat { 240 }

    var body: some View {
        VStack(alignment: .leading, spacing: Tokens.Space.s3) {
            if showsBackButton {
                BackButton()
            }
            content
        }
    }

    private var content: some View {
        HStack(alignment: .bottom, spacing: Tokens.Space.s6) {
            picture
            VStack(alignment: .leading, spacing: Tokens.Space.s2) {
                if let kind {
                    Text(kind.uppercased())
                        .lookFont(size: 11, weight: .bold)
                        .tracking(0.6)
                        .foregroundStyle(palette[Tokens.Ink.ink70])
                }
                Text(title)
                    .lookFont(size: 30, weight: .bold)
                    .foregroundStyle(palette[Tokens.Ink.ink90])
                    .lineLimit(2)
                    .accessibilityAddTraits(.isHeader)
                if let artist, !artist.isEmpty {
                    if let openArtist {
                        Button(action: openArtist) {
                            Text(artist)
                                .typography(Tokens.TypeRoles.nowPlayingArtist)
                                .foregroundStyle(palette[Tokens.Accent.accentText])
                        }
                        .buttonStyle(.plain)
                        .accessibilityAddTraits(.isLink)
                    } else {
                        Text(artist)
                            .typography(Tokens.TypeRoles.nowPlayingArtist)
                            .foregroundStyle(palette[Tokens.Accent.accentText])
                    }
                }
                if let facts, !facts.isEmpty {
                    Text(facts)
                        .typography(Tokens.TypeRoles.listSecondary)
                        .foregroundStyle(palette[Tokens.Ink.ink70])
                }
                if let credit {
                    Text(credit)
                        .typography(Tokens.TypeRoles.listSecondary)
                        .foregroundStyle(palette[Tokens.Ink.ink70])
                }
                HStack(spacing: Tokens.Space.s2) {
                    actions
                }
                .padding(.top, Tokens.Space.s2)
            }
            Spacer(minLength: 0)
        }
    }

    @ViewBuilder private var picture: some View {
        if round {
            ArtworkView(source: artwork, size: Self.artworkSide, cornerRadius: Self.artworkSide / 2,
                        placeholder: placeholder)
                .clipShape(Circle())
                .elevation(Tokens.Elevation.shadow2)
        } else {
            ArtworkView(source: artwork, size: Self.artworkSide, cornerRadius: Tokens.Artwork.radiusGrid,
                        placeholder: placeholder)
                .elevation(Tokens.Elevation.shadow2)
        }
    }
}

/// A short list of tracks outside a List (Home, an artist's songs, Search, a source's likes): a
/// double-click, the play button, Return on a focused row or the accessibility action plays the
/// list from the row; right-click and More open the song menu. With keyboard navigation on, Tab
/// focuses each row, which shows its play button and More as hovering does.
struct TrackStack: View {
    var items: [TrackItem]
    var style: TrackRowStyle = .full
    var play: (Int) -> Void

    @Environment(PlaybackStore.self) private var playback
    @FocusState private var focusedRow: TrackItem.ID?

    var body: some View {
        VStack(spacing: 0) {
            ForEach(Array(items.enumerated()), id: \.element.id) { index, item in
                TrackRowView(item: item, style: style, isCurrent: isCurrent(item), isPlaying: playback.isPlaying,
                             showsActions: focusedRow == item.id, play: { self.play(index) })
                    .padding(.horizontal, Tokens.Space.s2)
                    .focusable(interactions: .activate)
                    .focused($focusedRow, equals: item.id)
                    .onKeyPress(.return) {
                        mainAction(item, index)
                        return .handled
                    }
                    .onTapGesture(count: 2) { mainAction(item, index) }
                    .contextMenu { TrackActionsMenu(items: [item]) }
                    .accessibilityAddTraits(.isButton)
                    .accessibilityAction { mainAction(item, index) }
                if index < items.count - 1 {
                    Divider().opacity(0.5)
                }
            }
        }
    }

    private func isCurrent(_ item: TrackItem) -> Bool {
        item.track != nil && item.track == playback.nowPlaying?.track
    }

    /// Plays from the row; an item that cannot play here opens its link instead.
    private func mainAction(_ item: TrackItem, _ index: Int) {
        if item.canPlay {
            play(index)
        } else if item.opensLink {
            ServiceLink.open(item.link)
        }
    }
}

/// The context menu and double-click of a List of tracks with a selection (its rows' More opens the
/// same menu for one row; the selected rows show their play button and More).
struct TrackListActions: ViewModifier {
    var items: [TrackItem]
    /// Plays from the row at an index of `items`.
    var play: (Int) -> Void

    func body(content: Content) -> some View {
        content.contextMenu(forSelectionType: TrackItem.ID.self) { ids in
            TrackActionsMenu(items: items.filter { ids.contains($0.id) })
        } primaryAction: { ids in
            guard let id = ids.first, let index = items.firstIndex(where: { $0.id == id }) else { return }
            if items[index].canPlay {
                play(index)
            } else if items[index].opensLink {
                // It cannot play here: its main action opens it in its service.
                ServiceLink.open(items[index].link)
            }
        }
    }
}

extension View {
    func trackListActions(_ items: [TrackItem], play: @escaping (Int) -> Void) -> some View {
        modifier(TrackListActions(items: items, play: play))
    }
}

/// A centred state for an empty or failed page, with what to do about it (not while a failure
/// shows: the action would not help).
struct EmptyState<Actions: View>: View {
    var title: LocalizedStringKey
    var symbol: String
    var message: LocalizedStringKey?
    var error: String?
    @ViewBuilder var actions: Actions

    var body: some View {
        ContentUnavailableView {
            Label(title, systemImage: symbol)
        } description: {
            if let error {
                Text(error)
            } else if let message {
                Text(message)
            }
        } actions: {
            if error == nil {
                actions
            }
        }
    }
}

extension EmptyState where Actions == EmptyView {
    init(title: LocalizedStringKey, symbol: String, message: LocalizedStringKey? = nil, error: String? = nil) {
        self.init(title: title, symbol: symbol, message: message, error: error) { EmptyView() }
    }
}

/// Add Music Folder…: the folders chosen join the library (Home, the empty library pages).
struct AddMusicFolderButton: View {
    var prominent = false
    @Environment(LibraryStore.self) private var library

    var body: some View {
        let button = Button {
            FilePanels.chooseMusicFolders { urls in
                for url in urls { library.addFolder(url) }
            }
        } label: {
            Label("Add Music Folder…", systemImage: "folder.badge.plus")
        }
        if prominent {
            button.buttonStyle(.borderedProminent)
        } else {
            button
        }
    }
}

/// A button that scales on press (album covers 97 %, small buttons 92 %), on the Spring.
struct PressScaleStyle: ButtonStyle {
    var scale: CGFloat

    func makeBody(configuration: Configuration) -> some View {
        PressScaleBody(configuration: configuration, scale: scale)
    }
}

private struct PressScaleBody: View {
    let configuration: ButtonStyleConfiguration
    let scale: CGFloat
    @Environment(\.effects) private var effects

    var body: some View {
        configuration.label
            .scaleEffect(configuration.isPressed ? scale : 1)
            .animation(Motion.unless(!effects.animates, Motion.press), value: configuration.isPressed)
    }
}

/// "12 songs · 48 min" for a page header.
enum CountText {
    static func songs(_ count: Int) -> String {
        String(localized: "\(count) songs")
    }

    static func albums(_ count: Int) -> String {
        String(localized: "\(count) albums")
    }

    static func artists(_ count: Int) -> String {
        String(localized: "\(count) artists")
    }

    static func minutes(milliseconds: UInt64) -> String {
        let minutes = Int((milliseconds + 30_000) / 60_000)
        if minutes >= 60 {
            return String(localized: "\(minutes / 60) hr \(minutes % 60) min")
        }
        return String(localized: "\(minutes) min")
    }

    /// "48 minutes", "1 hour" or "1 hour 12 minutes" (the album page's footer).
    static func length(milliseconds: UInt64) -> String {
        let minutes = Int((milliseconds + 30_000) / 60_000)
        let hours = minutes / 60
        let rest = minutes % 60
        guard hours > 0 else { return String(localized: "\(minutes) minutes") }
        let hourText = String(localized: "\(hours) hours")
        guard rest > 0 else { return hourText }
        let minuteText = String(localized: "\(rest) minutes")
        return String(localized: "\(hourText) \(minuteText)")
    }

    /// "12 songs, 48 minutes".
    static func songsAndLength(count: Int, milliseconds: UInt64) -> String {
        let songText = songs(count)
        let lengthText = length(milliseconds: milliseconds)
        return String(localized: "\(songText), \(lengthText)")
    }

    static func joined(_ parts: [String?]) -> String {
        parts.compactMap { $0 }.filter { !$0.isEmpty }.joined(separator: " · ")
    }
}
