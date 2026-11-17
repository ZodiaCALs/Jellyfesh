// Generated JellyfeshFFI types to the app's types and back. Every generated name the app depends
// on appears in this folder; when the first build after a UniFFI change reports a renamed case or
// field, the fix belongs here (docs/macos.md lists the names assumed).

import JellyfeshFFI

// MARK: - References

extension TrackReference {
    init(_ ref: TrackRef) {
        switch ref {
        case .library(let id): self = .library(id: id)
        case .file(let path): self = .file(path: path)
        case .source(let track): self = .source(SourceSong(track))
        }
    }

    var ffi: TrackRef {
        switch self {
        case .library(let id): return .library(id: id)
        case .file(let path): return .file(path: path)
        case .source(let song): return .source(track: song.ffi)
        }
    }
}

extension SourceSong {
    init(_ track: SourceTrack) {
        self.init(plugin: track.plugin, id: track.id, title: track.title, artist: track.artist,
                  album: track.album, durationMs: track.durationMs, artworkURL: track.artworkUrl,
                  isrc: track.isrc)
    }

    var ffi: SourceTrack {
        SourceTrack(plugin: plugin, id: id, title: title, artist: artist, album: album,
                    durationMs: durationMs, artworkUrl: artworkURL, isrc: isrc)
    }
}

extension CollectionReference {
    var ffi: JellyfeshFFI.Collection {
        switch self {
        case .album(let id): return .album(id: id)
        case .artist(let id): return .artist(id: id)
        case .playlist(let id): return .playlist(id: id)
        case .favorites: return .favorites
        case .tracks(let tracks): return .tracks(tracks: tracks.map(\.ffi))
        case .source(let plugin, let id): return .source(plugin: plugin, id: id)
        }
    }
}

extension ArtworkLocation {
    init(_ ref: ArtworkRef) {
        switch ref {
        case .file(let path): self = .file(path: path)
        case .url(let url): self = .url(url)
        }
    }
}

// MARK: - Library

extension Song {
    init(_ t: Track) {
        self.init(id: t.id, path: t.path, title: t.title, artist: t.artist, album: t.album,
                  albumArtist: t.albumArtist, albumID: t.albumId, trackNumber: t.trackNo,
                  discNumber: t.discNo, year: t.year, genre: t.genre, durationMs: t.durationMs,
                  sampleRate: t.sampleRate, channels: t.channels, bitDepth: t.bitDepth, codec: t.codec,
                  isrc: t.isrc, artworkHash: t.artworkHash)
    }
}

extension AlbumSummary {
    init(_ a: Album) {
        self.init(id: a.id, title: a.title, albumArtist: a.albumArtist, year: a.year,
                  trackCount: a.trackCount, artworkHash: a.artworkHash)
    }
}

extension ArtistSummary {
    init(_ a: Artist) {
        self.init(id: a.id, name: a.name, trackCount: a.trackCount)
    }
}

extension SongRow {
    init(_ r: TrackRow) {
        self.init(track: TrackReference(r.track), title: r.title, artist: r.artist, album: r.album,
                  durationMs: r.durationMs, artwork: r.artwork.map(ArtworkLocation.init),
                  artworkHash: r.artworkHash, favorite: r.favorite, missing: r.missing)
    }
}

extension PlaylistInfo {
    init(_ p: PlaylistSummary) {
        self.init(id: p.id, name: p.name, entryCount: p.entryCount, durationMs: p.durationMs,
                  updatedAtMs: p.updatedAtMs, artworkHash: p.artworkHash)
    }
}

extension PlaylistRow {
    init(_ e: PlaylistEntry) {
        self.init(entryID: e.entryId, position: e.position, row: SongRow(e.row))
    }
}

extension SongSort {
    var ffi: TrackSort {
        switch self {
        case .artist: return .artist
        case .title: return .title
        case .album: return .album
        case .year: return .year
        case .duration: return .duration
        case .path: return .path
        case .added: return .added
        }
    }
}

extension AlbumOrder {
    var ffi: AlbumSort {
        switch self {
        case .title: return .title
        case .artist: return .artist
        case .year: return .year
        case .added: return .added
        }
    }
}

extension SongQuery {
    var ffi: TrackQuery {
        TrackQuery(offset: UInt32(clamping: offset), limit: UInt32(clamping: limit), sort: sort.ffi,
                   descending: descending, albumId: albumID, artistId: artistID)
    }
}

extension LibrarySearch {
    init(_ r: SearchResults) {
        self.init(songs: r.tracks.map(Song.init), albums: r.albums.map(AlbumSummary.init),
                  artists: r.artists.map(ArtistSummary.init))
    }
}

extension LibraryStatistics {
    init(_ s: LibraryStats) {
        self.init(roots: s.roots, tracks: s.tracks, albums: s.albums, artists: s.artists,
                  artwork: s.artwork, totalDurationMs: s.totalDurationMs, totalBytes: s.totalBytes)
    }
}

extension ScanState {
    init(_ s: ScanStatus) {
        switch s {
        case .progress(let p):
            self = .progress(filesSeen: p.filesSeen, filesChanged: p.filesChanged, current: p.current)
        case .finished(let r):
            self = .finished(ScanSummary(
                added: r.added, updated: r.updated, removed: r.removed, unchanged: r.unchanged,
                errors: r.errors.map { ScanIssue(path: $0.path, message: $0.message) }
            ))
        case .failed(let message):
            self = .failed(message)
        }
    }
}

// MARK: - Playback

extension PlaybackStatus {
    init(_ s: Status) {
        switch s {
        case .stopped: self = .stopped
        case .playing: self = .playing
        case .paused: self = .paused
        case .ended: self = .ended
        }
    }
}

extension Anchor {
    init(_ a: PlaybackAnchor) {
        self.init(trackGeneration: a.trackGeneration, positionUs: a.positionUs,
                  anchorMonoNs: a.anchorMonoNs, rate: a.rate, durationUs: a.durationUs)
    }
}

extension EqualizerBandKind {
    init(_ k: BandKind) {
        switch k {
        case .peaking: self = .peaking
        case .lowShelf: self = .lowShelf
        case .highShelf: self = .highShelf
        }
    }

    var ffi: BandKind {
        switch self {
        case .peaking: return .peaking
        case .lowShelf: return .lowShelf
        case .highShelf: return .highShelf
        }
    }
}

extension EqualizerConfig {
    init(_ e: EqSettings) {
        self.init(enabled: e.enabled, preampDb: e.preampDb, bands: e.bands.map {
            EqualizerBand(kind: EqualizerBandKind($0.kind), freqHz: $0.freqHz, gainDb: $0.gainDb,
                          q: $0.q, enabled: $0.enabled)
        })
    }

    var ffi: EqSettings {
        EqSettings(enabled: enabled, preampDb: preampDb, bands: bands.map {
            EqBand(kind: $0.kind.ffi, freqHz: $0.freqHz, gainDb: $0.gainDb, q: $0.q, enabled: $0.enabled)
        })
    }
}

extension EqualizerPreset {
    init(_ p: EqPreset) {
        self.init(id: p.id, settings: EqualizerConfig(p.settings))
    }
}

extension ReplayGainOption {
    init(_ m: ReplayGainMode) {
        switch m {
        case .off: self = .off
        case .track: self = .track
        case .album: self = .album
        }
    }

    var ffi: ReplayGainMode {
        switch self {
        case .off: return .off
        case .track: return .track
        case .album: return .album
        }
    }
}

extension PlaybackState {
    init(_ p: PlaybackSnapshot) {
        self.init(version: p.version, status: PlaybackStatus(p.status), anchor: Anchor(p.anchor),
                  volume: p.volume, eq: EqualizerConfig(p.eq), replayGain: ReplayGainOption(p.replaygainMode),
                  preampDb: p.preampDb)
    }
}

extension RepeatOption {
    init(_ m: RepeatMode) {
        switch m {
        case .off: self = .off
        case .all: self = .all
        case .one: self = .one
        }
    }

    var ffi: RepeatMode {
        switch self {
        case .off: return .off
        case .all: return .all
        case .one: return .one
        }
    }
}

extension QueueEntry {
    init(_ q: QueueItem) {
        self.init(uid: q.uid, track: TrackReference(q.track))
    }
}

extension QueueState {
    init(_ q: QueueSnapshot) {
        self.init(version: q.version, length: q.len, position: q.position,
                  current: q.current.map(QueueEntry.init), shuffle: q.shuffle,
                  repeatMode: RepeatOption(q.`repeat`), upNext: q.upNext.map(QueueEntry.init))
    }
}

extension NowPlayingInfo {
    init(_ n: NowPlaying) {
        self.init(uid: n.uid, track: TrackReference(n.track), path: n.path, title: n.title,
                  artist: n.artist, album: n.album, durationMs: n.durationMs,
                  artwork: n.artwork.map(ArtworkLocation.init), format: n.format.map(StreamFormat.init),
                  link: n.link)
    }
}

extension StreamFormat {
    init(_ f: TrackInfo) {
        self.init(codec: f.codec, sampleRate: f.sampleRate, channels: f.channels,
                  durationUs: f.durationUs, gapless: f.gapless, bitrate: f.bitrate)
    }
}

extension OutputDevice {
    init(_ d: DeviceInfo) {
        self.init(id: d.id, name: d.name, isDefault: d.isDefault, sampleRates: d.sampleRates)
    }
}

extension DevicesState {
    init(_ d: DevicesSnapshot) {
        self.init(version: d.version, devices: d.devices.map(OutputDevice.init), selected: d.selected)
    }
}

// MARK: - Lyrics

extension LyricsWord {
    init(_ w: Word) {
        self.init(startMs: w.startMs, endMs: w.endMs, text: w.text, joinsNext: w.joinsNext)
    }
}

extension LyricsLine {
    init(_ l: Line) {
        let direction: TextDirection
        switch l.direction {
        case .ltr: direction = .leftToRight
        case .rtl: direction = .rightToLeft
        case .auto: direction = .automatic
        }
        self.init(id: l.id, startMs: l.startMs, endMs: l.endMs, agent: l.agent, text: l.text,
                  words: l.words.map(LyricsWord.init), background: l.background.map(LyricsWord.init),
                  direction: direction)
    }
}

extension LyricsDocument {
    init(_ d: LyricDoc) {
        let timing: LyricsTiming
        switch d.timing {
        case .unsynced: timing = .unsynced
        case .line: timing = .line
        case .word: timing = .word
        }
        let format: String
        switch d.source.format {
        case .lrc: format = "LRC"
        case .enhancedLrc: format = "Enhanced LRC"
        case .ttml: format = "TTML"
        case .webVtt: format = "WebVTT"
        case .plain: format = "Text"
        }
        self.init(
            language: d.language,
            timing: timing,
            lines: d.lines.map(LyricsLine.init),
            tracks: d.tracks.map { track in
                LyricsAuxTrack(
                    kind: track.kind == .translation ? .translation : .romanization,
                    language: track.language,
                    lines: track.lines.mapValues(\.text)
                )
            },
            format: format,
            origin: d.source.origin
        )
    }
}

extension LyricsState {
    init(_ l: LyricsSnapshot) {
        let origin: LyricsOrigin?
        switch l.source {
        case .sidecar(let path)?: origin = .sidecar(path: path)
        case .embedded?: origin = .embedded
        // Lyrics from a plugin: its attribution is in the document's origin.
        case .plugin?: origin = nil
        case nil: origin = nil
        }
        self.init(uid: l.uid, document: l.doc.map(LyricsDocument.init),
                  timeline: l.timeline.map(LyricsTimeline.init), origin: origin)
    }
}

// MARK: - Theme

extension ThemePalette {
    init(_ t: Theme) {
        var colors: [String: UInt32] = [:]
        colors.reserveCapacity(t.colors.count)
        for color in t.colors {
            colors[color.name] = color.rgba
        }
        self.init(hue: t.hue, colors: colors)
    }
}

extension ThemeState {
    init(_ t: ThemeSnapshot) {
        self.init(uid: t.uid, hue: t.hue, fromArtwork: t.fromArtwork, light: ThemePalette(t.light),
                  dark: ThemePalette(t.dark), backdropLight: t.backdropLight, backdropDark: t.backdropDark)
    }
}

// MARK: - Settings

extension AppSettings {
    init(_ s: Settings) {
        let appearance: AppearanceOption
        switch s.appearance {
        case .system: appearance = .system
        case .light: appearance = .light
        case .dark: appearance = .dark
        }
        let hueSource: HueSourceOption
        switch s.hueSource {
        case .artwork: hueSource = .artwork
        case .fixed(let hue): hueSource = .fixed(hue: hue)
        }
        let lyricsSize: LyricsSizeOption
        switch s.lyricsSize {
        case .small: lyricsSize = .small
        case .medium: lyricsSize = .medium
        case .large: lyricsSize = .large
        }
        let lyricsAlign: LyricsAlignOption
        switch s.lyricsAlign {
        case .start: lyricsAlign = .start
        case .center: lyricsAlign = .center
        }
        self.init(
            appearance: appearance,
            style: s.style == .misty ? .misty : .artwork,
            appIcon: s.appIcon,
            hueSource: hueSource,
            reduceEffects: s.reduceEffects,
            transparency: s.transparency,
            lyricsSize: lyricsSize,
            lyricsAlign: lyricsAlign,
            showTranslation: s.showTranslation,
            showRomanization: s.showRomanization,
            crossfade: CrossfadeConfig(enabled: s.crossfade.enabled, durationMs: s.crossfade.durationMs,
                                       albumContinuity: s.crossfade.albumContinuity),
            replayGain: ReplayGainOption(s.replaygainMode),
            preampDb: s.preampDb,
            eq: EqualizerConfig(s.eq),
            outputDevice: s.outputDevice,
            resumeOnLaunch: s.resumeOnLaunch,
            watchFolders: s.watchFolders,
            allowCommunityPlugins: s.allowCommunityPlugins
        )
    }

    var ffi: Settings {
        let appearancePref: AppearancePref
        switch appearance {
        case .system: appearancePref = .system
        case .light: appearancePref = .light
        case .dark: appearancePref = .dark
        }
        let hue: HueSource
        switch hueSource {
        case .artwork: hue = .artwork
        case .fixed(let value): hue = .fixed(hue: value)
        }
        let size: LyricsSize
        switch lyricsSize {
        case .small: size = .small
        case .medium: size = .medium
        case .large: size = .large
        }
        let align: LyricsAlign
        switch lyricsAlign {
        case .start: align = .start
        case .center: align = .center
        }
        return Settings(
            appearance: appearancePref,
            style: style == .misty ? .misty : .artwork,
            appIcon: appIcon,
            hueSource: hue,
            reduceEffects: reduceEffects,
            transparency: transparency,
            lyricsSize: size,
            lyricsAlign: align,
            showTranslation: showTranslation,
            showRomanization: showRomanization,
            crossfade: crossfade.ffi,
            replaygainMode: replayGain.ffi,
            preampDb: preampDb,
            eq: eq.ffi,
            outputDevice: outputDevice,
            resumeOnLaunch: resumeOnLaunch,
            watchFolders: watchFolders,
            allowCommunityPlugins: allowCommunityPlugins
        )
    }
}

extension CrossfadeConfig {
    var ffi: CrossfadeSettings {
        CrossfadeSettings(enabled: enabled, durationMs: durationMs, albumContinuity: albumContinuity)
    }
}

// MARK: - Sources

extension SignInMethod {
    init(_ m: AuthMethod) {
        switch m {
        case .qr: self = .qr
        case .oauth: self = .oauth
        case .cookieImport: self = .cookieImport
        case .passwordExchange: self = .passwordExchange
        case .apiKey: self = .apiKey
        }
    }

    var ffi: AuthMethod {
        switch self {
        case .qr: return .qr
        case .oauth: return .oauth
        case .cookieImport: return .cookieImport
        case .passwordExchange: return .passwordExchange
        case .apiKey: return .apiKey
        }
    }
}

extension SourcePlugin {
    init(_ p: PluginSummary) {
        self.init(id: p.id, name: p.name, version: p.version, author: p.author,
                  badge: p.badge == .official ? .official : .community, vendorPlayback: p.vendorPlayback,
                  capabilities: p.capabilities, signInMethods: p.authMethods.map(SignInMethod.init),
                  enabled: p.enabled, account: p.account, needsDisclosure: p.needsDisclosure,
                  settingsSchemaJSON: p.settingsSchemaJson, settingsJSON: p.settingsJson, iconPath: p.icon,
                  remotePlayback: p.remotePlayback, attribution: p.attribution)
    }
}

extension SourcePreset {
    // The generated record has the same name: in this module `SourcePreset` is the app's own.
    init(_ p: JellyfeshFFI.SourcePreset) {
        self.init(id: p.id, name: p.name, plugin: p.plugin, signInMethods: p.authMethods.map(SignInMethod.init),
                  needsAddress: p.needsAddress, address: p.address, addressExample: p.addressExample,
                  account: p.account)
    }
}

extension SourcesState {
    init(_ s: SourcesSnapshot) {
        self.init(version: s.version, plugins: s.plugins.map(SourcePlugin.init),
                  presets: s.presets.map(SourcePreset.init), allowCommunity: s.allowCommunity)
    }
}

extension SignInUpdate {
    init(_ u: AuthUpdate) {
        let step: SignInStep
        switch u.step {
        case .showQr(let data, _, let expiresAtMs): step = .showQR(data: data, expiresAtMs: expiresAtMs)
        // A code to type on another device (Jellyfin Quick Connect), shown as text for now.
        case .showCode(let code, let hint, _): step = .waiting("\(hint): \(code)")
        case .openUrl(let url): step = .openURL(url)
        case .askFields(let fields):
            step = .askFields(fields.map {
                SignInField(name: $0.name, label: $0.label, secret: $0.secret, placeholder: $0.placeholder)
            })
        case .waiting(let message): step = .waiting(message)
        case .done(let account): step = .done(account: account)
        case .failed(let message): step = .failed(message)
        }
        self.init(plugin: u.plugin, flow: u.flow, step: step)
    }
}

extension ResultKind {
    init(_ k: ItemKind) {
        switch k {
        case .track: self = .track
        case .album: self = .album
        case .artist: self = .artist
        case .playlist: self = .playlist
        case .folder: self = .folder
        }
    }

    var ffi: ItemKind {
        switch self {
        case .track: return .track
        case .album: return .album
        case .artist: return .artist
        case .playlist: return .playlist
        case .folder: return .folder
        }
    }
}

extension SourceResult {
    init(_ i: SourceItem) {
        self.init(source: i.source, kind: ResultKind(i.kind), itemID: i.id, title: i.title,
                  subtitle: i.subtitle, album: i.album, durationMs: i.durationMs,
                  artwork: i.artwork.map(ArtworkLocation.init), playable: i.playable,
                  unplayableReason: i.unplayableReason, track: i.track.map(TrackReference.init),
                  alsoIn: i.alsoIn, link: i.link)
    }
}

extension SearchBatch {
    init(_ u: SearchUpdate) {
        self.init(request: u.request, query: u.query, source: u.source, items: u.items.map(SourceResult.init),
                  error: u.error, done: u.done)
    }
}

extension BrowseBatch {
    init(_ u: BrowseUpdate) {
        self.init(request: u.request, plugin: u.plugin, node: u.node, items: u.items.map(SourceResult.init),
                  nextPage: u.nextPage, error: u.error)
    }
}

// MARK: - Remote sessions

extension ServiceDeviceKind {
    init(_ k: RemoteDeviceKind) {
        switch k {
        case .computer: self = .computer
        case .smartphone: self = .smartphone
        case .speaker: self = .speaker
        case .tv: self = .tv
        case .car: self = .car
        case .other: self = .other
        }
    }

    var ffi: RemoteDeviceKind {
        switch self {
        case .computer: return .computer
        case .smartphone: return .smartphone
        case .speaker: return .speaker
        case .tv: return .tv
        case .car: return .car
        case .other: return .other
        }
    }
}

extension ServiceDevice {
    init(_ d: RemoteDevice) {
        self.init(id: d.id, name: d.name, kind: ServiceDeviceKind(d.kind), active: d.active,
                  volumePercent: d.volumePercent)
    }

    var ffi: RemoteDevice {
        RemoteDevice(id: id, name: name, kind: kind.ffi, active: active, volumePercent: volumePercent)
    }
}

extension RemoteSession {
    init(_ r: RemoteSnapshot) {
        self.init(version: r.version, source: r.source, sourceName: r.sourceName, status: PlaybackStatus(r.status),
                  item: r.item.map(SourceResult.init), context: r.context.map(SourceResult.init),
                  anchor: Anchor(r.anchor), shuffle: r.shuffle, repeatMode: RepeatOption(r.`repeat`),
                  device: r.device.map(ServiceDevice.init), devices: r.devices.map(ServiceDevice.init),
                  error: r.error)
    }
}

extension RemoteRequest {
    var ffi: RemoteAction {
        switch self {
        case .refresh: return .refresh
        case .transfer(let device): return .transfer(device: device)
        case .setVolume(let percent): return .setVolume(percent: percent)
        case .end: return .end
        }
    }
}

extension ExternalMedia {
    init(_ i: ExternalItem) {
        self.init(kind: ResultKind(i.kind), id: i.id, title: i.title, artist: i.artist, album: i.album,
                  durationMs: i.durationMs, artworkURL: i.artworkUrl)
    }

    var ffi: ExternalItem {
        ExternalItem(kind: kind.ffi, id: id, title: title, artist: artist, album: album, durationMs: durationMs,
                     artworkUrl: artworkURL)
    }
}

extension ExternalPlayerReport {
    var ffi: ExternalState {
        ExternalState(item: item?.ffi, context: context?.ffi, playing: playing, positionMs: positionMs,
                      shuffle: shuffle, `repeat`: repeatMode.ffi, device: device?.ffi)
    }
}

extension SearchArea {
    var ffi: SearchScope {
        switch self {
        case .all: return .all
        case .library: return .library
        case .plugin(let id): return .plugin(id: id)
        }
    }
}

// MARK: - Snapshot, events, commands

extension CoreState {
    init(_ s: Snapshot) {
        self.init(playback: PlaybackState(s.playback), queue: QueueState(s.queue),
                  nowPlaying: s.nowPlaying.map(NowPlayingInfo.init), lyrics: s.lyrics.map(LyricsState.init),
                  theme: s.theme.map(ThemeState.init), libraryRevision: s.libraryRevision,
                  scanning: s.scanning, settings: AppSettings(s.settings), devices: DevicesState(s.devices),
                  sources: SourcesState(s.sources), muted: s.muted, remote: RemoteSession(s.remote))
    }
}

extension CoreEvent {
    init(_ e: Event) {
        switch e {
        case .playback(let snapshot): self = .playback(PlaybackState(snapshot))
        case .queue(let snapshot): self = .queue(QueueState(snapshot))
        case .nowPlaying(let snapshot): self = .nowPlaying(snapshot.map(NowPlayingInfo.init))
        case .lyrics(let snapshot): self = .lyrics(LyricsState(snapshot))
        case .theme(let snapshot): self = .theme(ThemeState(snapshot))
        case .scan(let status): self = .scan(ScanState(status))
        case .libraryChanged(let revision): self = .libraryChanged(revision: revision)
        case .playlistCreated(let request, let id): self = .playlistCreated(request: request, id: id)
        case .settings(let snapshot): self = .settings(AppSettings(snapshot))
        case .devices(let snapshot): self = .devices(DevicesState(snapshot))
        case .sources(let snapshot): self = .sources(SourcesState(snapshot))
        case .remote(let snapshot): self = .remote(RemoteSession(snapshot))
        case .auth(let update): self = .signIn(SignInUpdate(update))
        case .searchResults(let update): self = .searchResults(SearchBatch(update))
        case .browsePage(let update): self = .browsePage(BrowseBatch(update))
        case .buffering(let uid, let buffering): self = .buffering(uid: uid, buffering: buffering)
        case .commandFailed(let id, let error): self = .commandFailed(id: id, message: error)
        case .error(let message): self = .error(message)
        }
    }
}

extension CoreCommand {
    var ffi: Command {
        switch self {
        case .play: return .play
        case .pause: return .pause
        case .togglePlayPause: return .togglePlayPause
        case .stop: return .stop
        case .next: return .next
        case .previous: return .previous
        case .seek(let positionMs): return .seek(positionMs: positionMs)
        case .setVolume(let volume): return .setVolume(volume: volume)
        case .setMuted(let muted): return .setMuted(muted: muted)
        case .playTracks(let tracks, let start): return .playTracks(tracks: tracks.map(\.ffi), start: start)
        case .playCollection(let collection, let start, let shuffle):
            return .playCollection(collection: collection.ffi, start: start, shuffle: shuffle)
        case .enqueue(let tracks, let playNext): return .enqueue(tracks: tracks.map(\.ffi), playNext: playNext)
        case .enqueueCollection(let collection, let playNext):
            return .enqueueCollection(collection: collection.ffi, playNext: playNext)
        case .removeFromQueue(let uid): return .removeFromQueue(uid: uid)
        case .moveInQueue(let uid, let to): return .moveInQueue(uid: uid, to: to)
        case .jumpTo(let uid): return .jumpTo(uid: uid)
        case .clearQueue: return .clearQueue
        case .setShuffle(let on): return .setShuffle(on: on)
        case .setRepeat(let mode): return .setRepeat(mode: mode.ffi)
        case .setSettings(let settings): return .setSettings(settings: settings.ffi)
        case .setEq(let eq): return .setEq(eq: eq.ffi)
        case .setReplayGain(let mode, let preampDb): return .setReplayGain(mode: mode.ffi, preampDb: preampDb)
        case .setCrossfade(let crossfade): return .setCrossfade(crossfade: crossfade.ffi)
        case .setOutputDevice(let name): return .setOutputDevice(name: name)
        case .refreshDevices: return .refreshDevices
        case .addRoot(let path): return .addRoot(path: path)
        case .removeRoot(let path): return .removeRoot(path: path)
        case .rescan: return .rescan
        case .createPlaylist(let name, let tracks): return .createPlaylist(name: name, tracks: tracks.map(\.ffi))
        case .renamePlaylist(let id, let name): return .renamePlaylist(id: id, name: name)
        case .deletePlaylist(let id): return .deletePlaylist(id: id)
        case .addToPlaylist(let id, let tracks): return .addToPlaylist(id: id, tracks: tracks.map(\.ffi))
        case .removeFromPlaylist(let id, let entries): return .removeFromPlaylist(id: id, entries: entries)
        case .movePlaylistEntry(let id, let entry, let to): return .movePlaylistEntry(id: id, entry: entry, to: to)
        case .importPlaylist(let path): return .importPlaylist(path: path)
        case .setFavorite(let track, let favorite): return .setFavorite(track: track.ffi, favorite: favorite)
        case .installPlugin(let path): return .installPlugin(path: path)
        case .uninstallPlugin(let id): return .uninstallPlugin(id: id)
        case .setPluginEnabled(let id, let enabled): return .setPluginEnabled(id: id, enabled: enabled)
        case .setPluginSettings(let id, let json): return .setPluginSettings(id: id, settingsJson: json)
        case .acknowledgeUnofficial(let id): return .acknowledgeUnofficial(id: id)
        case .beginLogin(let plugin, let method): return .beginLogin(plugin: plugin, method: method.ffi)
        case .connectSource(let preset, let address, let method):
            return .connectSource(preset: preset, address: address, method: method.ffi)
        case .submitLogin(let plugin, let flow, let fields): return .submitLogin(plugin: plugin, flow: flow, fields: fields)
        case .cancelLogin(let plugin, let flow): return .cancelLogin(plugin: plugin, flow: flow)
        case .logout(let plugin): return .logout(plugin: plugin)
        case .search(let query, let area): return .search(query: query, scope: area.ffi)
        case .browse(let plugin, let node, let page): return .browse(plugin: plugin, node: node, page: page)
        case .remote(let request): return .remote(action: request.ffi)
        }
    }
}

extension CoreClientError {
    init(_ error: Error) {
        guard let ffi = error as? CoreFfiError else {
            self = .core(String(describing: error))
            return
        }
        // UniFFI flat errors keep the Rust variant names (`CoreFfiError.NotAvailable(message:)`).
        switch ffi {
        case .Core(let message): self = .core(message)
        case .NotAvailable(let message): self = .notAvailable(message)
        case .ShutDown: self = .shutDown
        case .InvalidArgument(let message): self = .invalidArgument(message)
        }
    }
}
