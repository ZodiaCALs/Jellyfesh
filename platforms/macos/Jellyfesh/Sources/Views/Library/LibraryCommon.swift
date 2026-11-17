// Pieces the library pages share: album cards, a track list that plays on double-click or Return,
// and the empty and loading states.

import SwiftUI

/// An album in a grid or shelf: artwork (`artwork.radius-grid`, Luminous 16 pt on `shadow-2`),
/// title and artist. Opens the album. Hovering or focusing lifts the cover 4 pt onto `shadow-3`
/// and shows a 40 pt play button in its bottom start corner; a press scales it to 97 %.
struct AlbumCard: View {
    var album: AlbumSummary
    /// Artwork side in points; nil fills the grid cell.
    var size: CGFloat?

    @Environment(Router.self) private var router
    @Environment(PlaybackStore.self) private var playback
    @Environment(\.palette) private var palette
    @Environment(\.effects) private var effects
    @State private var hovering = false
    @FocusState private var focused: Bool

    var body: some View {
        let lifted = hovering || focused
        VStack(alignment: .leading, spacing: Tokens.Space.s2) {
            Button {
                router.open(.album(album))
            } label: {
                ArtworkView(source: album.artworkHash.map(ArtworkSource.hash), size: size,
                            cornerRadius: Tokens.Artwork.radiusGrid, placeholder: "square.stack")
                    .elevation(lifted ? Tokens.Elevation.shadow3 : Tokens.Elevation.shadow2)
            }
            .buttonStyle(PressScaleStyle(scale: 0.97))
            .focused($focused)
            .overlay(alignment: .bottomLeading) {
                if hovering {
                    Button {
                        playback.play(.album(id: album.id))
                    } label: {
                        Image(systemName: "play.fill")
                            .font(.system(size: 15, weight: .semibold))
                            .foregroundStyle(palette[Tokens.Ink.ink90])
                            .frame(width: 40, height: 40)
                            .surface(in: Circle())
                            .contentShape(Circle())
                    }
                    .buttonStyle(PressScaleStyle(scale: 0.92))
                    .padding(Tokens.Space.s2)
                    .accessibilityLabel(Text("Play \(album.title)"))
                    .help(Text("Play \(album.title)"))
                    .transition(.opacity)
                }
            }
            .offset(y: lifted && effects.animates ? -LuminousGeometry.coverLift : 0)
            .animation(Motion.unless(!effects.animates, Motion.hover), value: lifted)
            VStack(alignment: .leading, spacing: 1) {
                Text(album.title.isEmpty ? String(localized: "Unknown Album") : album.title)
                    .typography(Tokens.TypeRoles.listTitle)
                    .foregroundStyle(palette[Tokens.Ink.ink90])
                    .lineLimit(1)
                Text(album.albumArtist ?? String(localized: "Unknown Artist"))
                    .typography(Tokens.TypeRoles.listSecondary)
                    .foregroundStyle(palette[Tokens.Ink.ink70])
                    .lineLimit(1)
            }
        }
        .frame(width: size)
        .onHover { hovering = $0 }
        .contextMenu {
            Button("Play") { playback.play(.album(id: album.id)) }
            Button("Shuffle") { playback.play(.album(id: album.id), shuffle: true) }
            Divider()
            Button("Play Next") { playback.enqueue(.album(id: album.id), next: true) }
            Button("Play Last") { playback.enqueue(.album(id: album.id), next: false) }
            AddToPlaylistMenu(collection: .album(id: album.id))
        }
        .accessibilityElement(children: .combine)
        .accessibilityLabel(Text(album.albumArtist.map { "\(album.title), \($0)" } ?? album.title))
        .accessibilityAddTraits(.isButton)
        .accessibilityAction { router.open(.album(album)) }
    }
}

/// A short list of tracks outside a List (Home): double-click or the accessibility action plays
/// the list from the row.
struct TrackStack: View {
    var items: [TrackItem]
    var style: TrackRowStyle = .full
    var play: (Int) -> Void

    @Environment(PlaybackStore.self) private var playback

    var body: some View {
        VStack(spacing: 0) {
            ForEach(Array(items.enumerated()), id: \.element.id) { index, item in
                TrackRowView(item: item, style: style, isCurrent: isCurrent(item), isPlaying: playback.isPlaying)
                    .padding(.horizontal, Tokens.Space.s2)
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

/// The context menu and double-click of a List of tracks with a selection.
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
                for url in urls { library.addFolder(url.path) }
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
