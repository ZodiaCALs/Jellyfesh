// Apple Music in Sources (ADR-0010, ADR-0018 item 5): offered in Add a Source only to a copy of the
// app that MusicKit can work in, turned on only after the person agrees, then listed in Your
// Sources. Its page searches the catalogue (songs and albums); they play through MusicKit's player
// as a remote session, never joining the queue, playlists or favourites. Results stay in memory
// and in their own group; the Search page shows them as their own Apple Music group too.

import SwiftUI

/// Apple Music in Add a Source, while it can be turned on: asks first, then turns it on.
struct AppleMusicOfferRow: View {
    var iconSize: CGFloat = 40

    @Environment(AppleMusicStore.self) private var appleMusic
    @Environment(\.palette) private var palette
    @State private var asking = false

    var body: some View {
        Button {
            asking = true
        } label: {
            HStack(spacing: Tokens.Space.s3) {
                AppleMusicIcon(size: iconSize)
                VStack(alignment: .leading, spacing: 1) {
                    Text(verbatim: AppleMusicStore.name)
                        .typography(Tokens.TypeRoles.listTitle)
                        .foregroundStyle(palette[Tokens.Ink.ink90])
                    Text("Your Apple Music subscription, on this Mac")
                        .typography(Tokens.TypeRoles.listSecondary)
                        .foregroundStyle(palette[Tokens.Ink.ink70])
                }
                Spacer(minLength: 0)
                if appleMusic.availability == .checking {
                    ProgressView()
                        .controlSize(.small)
                } else {
                    Image(systemName: "chevron.right")
                        .font(.system(size: 11, weight: .semibold))
                        .foregroundStyle(palette[Tokens.Ink.ink30])
                        .accessibilityHidden(true)
                }
            }
            .frame(minHeight: Tokens.Player.rowHeight)
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .disabled(appleMusic.availability != .offered)
        .confirmationDialog(Text("Turn On Apple Music?"), isPresented: $asking, titleVisibility: .visible) {
            Button("Turn On") {
                Task { await appleMusic.turnOn() }
            }
        } message: {
            Text("Jellyfesh asks macOS for access to Apple Music, then plays its songs with Apple's own player on this Mac. Nothing from Apple Music is kept in your library.")
        }
    }
}

/// Apple Music in Your Sources: opens its page.
struct AppleMusicSourceRow: View {
    @Environment(\.palette) private var palette

    var body: some View {
        HStack(spacing: Tokens.Space.s3) {
            AppleMusicIcon(size: 40)
            VStack(alignment: .leading, spacing: 1) {
                Text(verbatim: AppleMusicStore.name)
                    .typography(Tokens.TypeRoles.listTitle)
                    .foregroundStyle(palette[Tokens.Ink.ink90])
                Text("Plays on this Mac")
                    .typography(Tokens.TypeRoles.listSecondary)
                    .foregroundStyle(palette[Tokens.Ink.ink70])
            }
            Spacer(minLength: 0)
        }
        .frame(height: Tokens.Player.rowHeight)
        .accessibilityElement(children: .combine)
    }
}

/// A plain glyph for Apple Music: no logos until they are approved (docs/ui.md).
private struct AppleMusicIcon: View {
    var size: CGFloat
    @Environment(\.palette) private var palette

    var body: some View {
        Image(systemName: "music.note")
            .font(.system(size: size * 0.45, weight: .semibold))
            .foregroundStyle(palette[Tokens.Ink.ink70])
            .frame(width: size, height: size)
            .background(RoundedRectangle(cornerRadius: Tokens.Radius.rMd, style: .continuous)
                .fill(palette[Tokens.Ink.ink06]))
            .accessibilityHidden(true)
    }
}

/// The Apple Music page: the catalogue's search, and Turn Off.
struct AppleMusicView: View {
    @Environment(AppleMusicStore.self) private var appleMusic
    @Environment(Router.self) private var router
    @Environment(\.palette) private var palette
    @State private var text = ""
    @FocusState private var fieldFocused: Bool

    var body: some View {
        let search = appleMusic.pageSearch
        ScrollView {
            VStack(alignment: .leading, spacing: Tokens.Space.s6) {
                PageHeader(title: AppleMusicStore.name,
                           subtitle: String(localized: "Plays with your subscription on this Mac. Nothing from Apple Music is kept in your library."))
                if appleMusic.isAvailable {
                    HStack(spacing: Tokens.Space.s2) {
                        Image(systemName: "magnifyingglass")
                            .foregroundStyle(palette[Tokens.Ink.ink50])
                        TextField("Songs and albums", text: $text)
                            .textFieldStyle(.plain)
                            .font(.system(size: 17))
                            .focused($fieldFocused)
                            .onSubmit { Task { await search.run(text, using: appleMusic.catalog) } }
                            .accessibilityLabel(Text("Search Apple Music"))
                        if search.isSearching {
                            ProgressView().controlSize(.small)
                        }
                    }
                    .padding(.horizontal, Tokens.Space.s4)
                    .frame(height: 44)
                    .surface(in: Capsule())
                    .frame(maxWidth: 640)

                    if search.query.isEmpty {
                        EmptyState(title: "Search Apple Music", symbol: "magnifyingglass",
                                   message: "Find songs and albums in the Apple Music catalogue.")
                            .frame(maxWidth: .infinity, minHeight: 280)
                    } else {
                        AppleMusicResultsView(search: search, showsTitle: false)
                    }

                    HStack {
                        Button("Turn Off Apple Music") {
                            appleMusic.turnOff()
                            router.back()
                        }
                        .buttonStyle(.bordered)
                        Spacer(minLength: 0)
                    }
                    .padding(.top, Tokens.Space.s4)
                } else {
                    EmptyState(title: "Apple Music Is Off", symbol: "music.note",
                               message: "Turn it on in Sources › Add a Source.")
                        .frame(maxWidth: .infinity, minHeight: 280)
                }
            }
            .padding(.horizontal, Tokens.Space.s7)
            .padding(.vertical, Tokens.Space.s6)
            .frame(maxWidth: 1100, alignment: .leading)
        }
        .navigationTitle(Text(verbatim: AppleMusicStore.name))
        .task(id: text) {
            // Searches after a short pause in typing.
            try? await Task.sleep(for: .milliseconds(350))
            guard !Task.isCancelled, text.trimmingCharacters(in: .whitespacesAndNewlines) != search.query else { return }
            await search.run(text, using: appleMusic.catalog)
        }
        .onAppear { fieldFocused = true }
    }
}

/// Apple Music's results: songs (they play from the one chosen) and albums (they play whole), in
/// their own group. Each has Open in Apple Music; none joins the queue, playlists or favourites.
struct AppleMusicResultsView: View {
    var search: AppleMusicSearch
    /// The group's title, on the Search page.
    var showsTitle: Bool

    @Environment(PlaybackStore.self) private var playback
    @Environment(\.palette) private var palette

    private let columns = [
        GridItem(.adaptive(minimum: 150, maximum: 200), spacing: Tokens.Space.s5, alignment: .top),
    ]

    var body: some View {
        let results = search.results
        if !search.query.isEmpty {
            VStack(alignment: .leading, spacing: Tokens.Space.s4) {
                if showsTitle && (!results.isEmpty || search.error != nil) {
                    Text(verbatim: AppleMusicStore.name)
                        .font(.system(size: 22, weight: .bold))
                        .foregroundStyle(palette[Tokens.Ink.ink90])
                        .accessibilityAddTraits(.isHeader)
                }
                if let error = search.error {
                    Text(error)
                        .font(.system(size: 13))
                        .foregroundStyle(palette[Tokens.Semantic.danger])
                } else if results.isEmpty && !search.isSearching && !showsTitle {
                    EmptyState(title: "No Results", symbol: "magnifyingglass",
                               message: "Try other words, or check the spelling.")
                        .frame(maxWidth: .infinity, minHeight: 240)
                }
                if !results.songs.isEmpty {
                    VStack(alignment: .leading, spacing: Tokens.Space.s2) {
                        Text("Songs")
                            .font(.system(size: 13, weight: .semibold))
                            .foregroundStyle(palette[Tokens.Ink.ink70])
                        let items = results.songs.map(Self.item)
                        TrackStack(items: items) { index in
                            playback.play(tracks: results.songs.map(\.track), from: index)
                        }
                        .surface(in: RoundedRectangle(cornerRadius: surfaceRadius, style: .continuous))
                    }
                }
                if !results.albums.isEmpty {
                    VStack(alignment: .leading, spacing: Tokens.Space.s2) {
                        Text("Albums")
                            .font(.system(size: 13, weight: .semibold))
                            .foregroundStyle(palette[Tokens.Ink.ink70])
                        LazyVGrid(columns: columns, alignment: .leading, spacing: Tokens.Space.s5) {
                            ForEach(results.albums) { album in
                                AppleMusicAlbumCard(album: album)
                            }
                        }
                    }
                }
            }
        }
    }

    static func item(_ song: AppleMusicSong) -> TrackItem {
        TrackItem(id: "am-\(song.id)", track: song.track, title: song.title, artist: song.artist, album: song.album,
                  durationMs: song.durationMs, artwork: song.artworkURL.map(ArtworkSource.url), link: song.link,
                  source: AppleMusicIDs.source)
    }
}

/// An Apple Music album: plays it whole (there is no album page for the catalogue).
private struct AppleMusicAlbumCard: View {
    var album: AppleMusicAlbum

    @Environment(PlaybackStore.self) private var playback
    @Environment(\.palette) private var palette

    var body: some View {
        Button {
            playback.play(album.collection)
        } label: {
            VStack(alignment: .leading, spacing: Tokens.Space.s2) {
                ArtworkView(source: album.artworkURL.map(ArtworkSource.url), size: nil,
                            cornerRadius: Tokens.Artwork.radiusGrid, placeholder: "square.stack")
                    .elevation(Tokens.Elevation.shadow2)
                Text(album.title)
                    .typography(Tokens.TypeRoles.listTitle)
                    .foregroundStyle(palette[Tokens.Ink.ink90])
                    .lineLimit(1)
                if let artist = album.artist {
                    Text(artist)
                        .typography(Tokens.TypeRoles.listSecondary)
                        .foregroundStyle(palette[Tokens.Ink.ink70])
                        .lineLimit(1)
                }
            }
        }
        .buttonStyle(PressScaleStyle(scale: 0.97))
        .contextMenu {
            Button("Play") { playback.play(album.collection) }
            Button("Shuffle") { playback.play(album.collection, shuffle: true) }
            OpenInServiceButton(link: album.link, source: AppleMusicIDs.source)
        }
        .accessibilityLabel(Text(album.artist.map { "\(album.title), \($0)" } ?? album.title))
        .accessibilityHint(Text("Plays the album"))
    }
}
