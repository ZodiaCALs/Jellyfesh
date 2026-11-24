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
        case .songs(let sort, let descending): return .songs(sort: sort.ffi, descending: descending)
        case .tracks(let tracks): return .tracks(tracks: tracks.map(\.ffi))
        case .source(let plugin, let id, let kind): return .source(plugin: plugin, id: id, itemKind: kind.ffi)
        }
    }
}

extension SongReference {
    var ffi: SongRef {
        switch self {
        case .current: return .current
        case .library(let id): return .library(id: id)
        }
    }
}

extension ItemReference {
    init(_ ref: ItemRef) {
        switch ref {
        case .track(let id): self = .track(id: id)
        case .album(let id): self = .album(id: id)
        case .artist(let id): self = .artist(id: id)
        case .file(let path): self = .file(path: path)
        case .source(let plugin, let itemKind, let id): self = .source(plugin: plugin, kind: ResultKind(itemKind), id: id)
        }
    }

    var ffi: ItemRef {
        switch self {
        case .track(let id): return .track(id: id)
        case .album(let id): return .album(id: id)
        case .artist(let id): return .artist(id: id)
        case .file(let path): return .file(path: path)
        case .source(let plugin, let kind, let id): return .source(plugin: plugin, itemKind: kind.ffi, id: id)
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
                  link: n.link, quality: n.quality.map(AudioQuality.init),
                  qualities: n.qualities.map(StreamingQualityOption.init),
                  qualityNote: n.qualityNote.map(QualityNoteOption.init))
    }
}

extension StreamingQualityOption {
    init(_ q: StreamingQuality) {
        switch q {
        case .auto: self = .auto
        case .lossless: self = .lossless
        case .high: self = .high
        case .normal: self = .normal
        case .low: self = .low
        }
    }

    var ffi: StreamingQuality {
        switch self {
        case .auto: return .auto
        case .lossless: return .lossless
        case .high: return .high
        case .normal: return .normal
        case .low: return .low
        }
    }
}

extension QualityNoteOption {
    init(_ n: QualityNote) {
        switch n {
        case .filesPlayAsTheyAre: self = .filesPlayAsTheyAre
        case .setInService(let service): self = .setInService(service)
        }
    }
}

extension StreamFormat {
    init(_ f: TrackInfo) {
        self.init(codec: f.codec, sampleRate: f.sampleRate, channels: f.channels,
                  durationUs: f.durationUs, gapless: f.gapless, bitrate: f.bitrate, bitsPerSample: f.bitsPerSample)
    }
}

extension AudioQualityLabel {
    init(_ q: QualityLabel) {
        switch q {
        case .hiResLossless: self = .hiResLossless
        case .lossless: self = .lossless
        case .high: self = .high
        case .standard: self = .standard
        }
    }
}

extension AudioQuality {
    init(_ f: AudioFormat) {
        self.init(codec: f.codec, sampleRateHz: f.sampleRateHz, bitDepth: f.bitDepth, channels: f.channels,
                  bitrateKbps: f.bitrateKbps, lossless: f.lossless, label: f.quality.map(AudioQualityLabel.init))
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
        case .made(let plugin, let engine)?: origin = .made(plugin: plugin, engine: engine)
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
            allowCommunityPlugins: s.allowCommunityPlugins,
            checkForUpdates: s.checkForUpdates,
            showLyricsBar: s.showLyricsBar,
            desktopLyrics: s.desktopLyrics,
            streamingQuality: StreamingQualityOption(s.streamingQuality),
            downloads: DownloadOptions(s.downloads),
            miniPlayer: MiniPlayerOptions(s.miniPlayer)
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
            allowCommunityPlugins: allowCommunityPlugins,
            checkForUpdates: checkForUpdates,
            showLyricsBar: showLyricsBar,
            desktopLyrics: desktopLyrics,
            streamingQuality: streamingQuality.ffi,
            downloads: downloads.ffi,
            miniPlayer: miniPlayer.ffi
        )
    }
}

extension DownloadQualityOption {
    init(_ q: DownloadQuality) {
        switch q {
        case .original: self = .original
        case .high: self = .high
        case .normal: self = .normal
        case .low: self = .low
        }
    }

    var ffi: DownloadQuality {
        switch self {
        case .original: return .original
        case .high: return .high
        case .normal: return .normal
        case .low: return .low
        }
    }
}

extension DownloadOptions {
    init(_ d: DownloadSettings) {
        self.init(folder: d.folder, quality: DownloadQualityOption(d.quality), nameTemplate: d.nameTemplate,
                  addMissingCoverAndLyrics: d.addMissingCoverAndLyrics, lyricsInTags: d.lyricsInTags,
                  saveLrcFiles: d.saveLrcFiles, wifiOnly: d.wifiOnly, atOnce: d.atOnce)
    }

    var ffi: DownloadSettings {
        DownloadSettings(folder: folder, quality: quality.ffi, nameTemplate: nameTemplate,
                         addMissingCoverAndLyrics: addMissingCoverAndLyrics, lyricsInTags: lyricsInTags,
                         saveLrcFiles: saveLrcFiles, wifiOnly: wifiOnly, atOnce: atOnce)
    }
}

extension MiniPlayerOptions {
    init(_ m: MiniPlayerSettings) {
        self.init(keepOnTop: m.keepOnTop, openWhenWindowClosed: m.openWhenWindowClosed,
                  showLyricsLine: m.showLyricsLine, androidPip: m.androidPip)
    }

    var ffi: MiniPlayerSettings {
        MiniPlayerSettings(keepOnTop: keepOnTop, openWhenWindowClosed: openWhenWindowClosed,
                           showLyricsLine: showLyricsLine, androidPip: androidPip)
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
                  remotePlayback: p.remotePlayback, attribution: p.attribution, isExtension: p.kind == .`extension`,
                  requiresPro: p.requiresPro, permissions: p.permissions, hasDiscover: p.discover,
                  hasDetails: p.details, hasComments: p.comments, summary: p.description, hasLibrary: p.library,
                  offersDownloads: p.download, qualities: p.qualities.map(StreamingQualityOption.init))
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
                  presets: s.presets.map(SourcePreset.init), allowCommunity: s.allowCommunity,
                  canInstallPlugins: s.canInstallPlugins)
    }
}

extension SignInFieldKind {
    init(_ k: AuthFieldKind) {
        switch k {
        case .text: self = .text
        case .email: self = .email
        case .url: self = .url
        case .number: self = .number
        case .cookieHeader: self = .cookieHeader
        }
    }
}

extension SignInField {
    init(_ f: AuthField) {
        self.init(name: f.name, label: f.label, kind: SignInFieldKind(f.kind), secret: f.secret,
                  required: f.required, placeholder: f.placeholder, help: f.help)
    }
}

extension SignInUpdate {
    init(_ u: AuthUpdate) {
        let step: SignInStep
        switch u.step {
        case .showQr(let data, let hint, let expiresAtMs):
            step = .showQR(data: data, hint: hint, expiresAtMs: expiresAtMs)
        case .showCode(let code, let hint, let expiresAtMs):
            step = .showCode(code: code, hint: hint, expiresAtMs: expiresAtMs)
        case .openUrl(let url): step = .openURL(url)
        case .askFields(let fields): step = .askFields(fields.map { SignInField($0) })
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
                  alsoIn: i.alsoIn, link: i.link, artistID: i.artistId, albumID: i.albumId,
                  downloaded: i.downloaded)
    }
}

extension SearchPageBatch {
    init(_ p: SearchPage) {
        self.init(request: p.request, query: p.query, source: p.source, kind: ResultKind(p.kind),
                  items: p.items.map(SourceResult.init), next: p.next, error: p.error)
    }
}

// MARK: - The sources' likes and playlists, and downloads

extension SourceListPart {
    init(_ p: SourceLibraryPart) {
        switch p {
        case .likedSongs: self = .likedSongs
        case .likedAlbums: self = .likedAlbums
        case .likedArtists: self = .likedArtists
        case .playlists: self = .playlists
        }
    }

    var ffi: SourceLibraryPart {
        switch self {
        case .likedSongs: return .likedSongs
        case .likedAlbums: return .likedAlbums
        case .likedArtists: return .likedArtists
        case .playlists: return .playlists
        }
    }
}

extension SourceListEntry {
    init(_ e: SourceLibraryEntry) {
        self.init(item: SourceResult(e.item), owned: e.owned, songsListed: e.songsListed)
    }
}

extension SourceList {
    init(_ l: SourceLibraryList) {
        self.init(part: SourceListPart(l.part), entries: l.entries.map(SourceListEntry.init), total: l.total,
                  next: l.next, refreshedAtMs: l.refreshedAtMs, loading: l.loading, error: l.error)
    }
}

extension SourceLibraryState {
    init(_ s: SourceLibrarySnapshot) {
        self.init(version: s.version, sources: s.sources.map { library in
            SourceLibraryInfo(plugin: library.plugin, name: library.name, attribution: library.attribution,
                              lists: library.lists.map(SourceList.init))
        })
    }
}

extension SourceLibraryPageBatch {
    init(_ p: SourceLibraryPage) {
        self.init(request: p.request, plugin: p.plugin, part: SourceListPart(p.part),
                  entries: p.entries.map(SourceListEntry.init), next: p.next, error: p.error)
    }
}

extension DownloadRequest {
    var ffi: DownloadTarget {
        switch self {
        case .track(let song): return .track(track: .source(track: song.ffi))
        case .collection(let plugin, let id, let kind): return .collection(plugin: plugin, id: id, itemKind: kind.ffi)
        }
    }
}

extension DownloadProblem {
    init(_ e: DownloadError) {
        switch e {
        case .notAllowed(let service): self = .notAllowed(service: service)
        case .notOffered(let service): self = .notOffered(service: service)
        case .network(let message): self = .network(message)
        case .storage(let message): self = .storage(message)
        case .other(let message): self = .other(message)
        }
    }
}

extension DownloadPhase {
    init(_ s: DownloadState) {
        switch s {
        case .queued: self = .queued
        case .downloading: self = .downloading
        case .done: self = .done
        case .failed(let error): self = .failed(DownloadProblem(error))
        }
    }
}

extension DownloadsState {
    init(_ s: DownloadsSnapshot) {
        self.init(version: s.version, folder: s.folder, items: s.items.map { item in
            DownloadEntry(id: item.id, track: SourceSong(item.track), phase: DownloadPhase(item.state),
                          bytesDone: item.bytesDone, bytesTotal: item.bytesTotal, path: item.path,
                          libraryID: item.libraryId, original: item.original)
        }, bytesUsed: s.bytesUsed, paused: s.paused.map { paused in
            switch paused {
            case .offline: return DownloadsPause.offline
            case .metered: return DownloadsPause.metered
            }
        })
    }
}

extension SearchBatch {
    init(_ u: SearchUpdate) {
        self.init(request: u.request, query: u.query, source: u.source, items: u.items.map(SourceResult.init),
                  error: u.error, done: u.done, merged: u.merged,
                  groups: u.groups.map { group in
                      SearchKindGroup(kind: ResultKind(group.kind), items: group.items.map(SourceResult.init),
                                      next: group.next, total: group.total)
                  })
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
                  error: r.error, qualityNote: r.qualityNote.map(QualityNoteOption.init))
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

// MARK: - Distribution and Pro

extension AppDistribution {
    var ffi: Distribution {
        switch self {
        case .direct: return .direct
        case .appStore: return .appStore
        case .source: return .source
        }
    }
}

extension ProShop {
    init(_ s: Store) {
        switch s {
        case .appStore: self = .appStore
        case .microsoftStore: self = .microsoftStore
        case .googlePlay: self = .googlePlay
        }
    }

    var ffi: Store {
        switch self {
        case .appStore: return .appStore
        case .microsoftStore: return .microsoftStore
        case .googlePlay: return .googlePlay
        }
    }
}

extension ProEntitlement {
    var ffi: StoreEntitlement {
        StoreEntitlement(store: store.ffi, product: product, active: active, transaction: transaction)
    }
}

extension ProState {
    init(_ p: ProSnapshot) {
        let origin: ProOrigin
        switch p.source {
        case .none: origin = .none
        case .preview: origin = .preview
        case .store(let store): origin = .store(ProShop(store))
        case .licence(let idSuffix, let issuedAtMs): origin = .licence(idSuffix: idSuffix, issuedAtMs: issuedAtMs)
        }
        let problem: LicenceProblem?
        switch p.licenceError {
        case .malformed?: problem = .malformed
        case .unknownKey?: problem = .unknownKey
        case .badSignature?: problem = .badSignature
        case .wrongProduct?: problem = .wrongProduct
        case .revoked?: problem = .revoked
        case nil: problem = nil
        }
        self.init(version: p.version, active: p.active, origin: origin, preview: p.preview, licenceProblem: problem,
                  stores: p.stores.map(ProShop.init), licenceKeys: p.licenceKeys)
    }
}

extension UpdateState {
    init(_ u: UpdateSnapshot) {
        self.init(version: u.version, supported: u.supported, checking: u.checking,
                  available: u.available.map { ReleaseInfo(version: $0.version, url: $0.url) },
                  lastCheckedMs: u.lastCheckedMs, auto: u.auto, error: u.error)
    }
}

// MARK: - AI Lyrics

extension AiLyricsState {
    init(_ a: AiLyricsSnapshot) {
        self.init(
            version: a.version,
            available: a.available,
            plugin: a.plugin,
            plan: a.plan.map { plan in
                LyricsMakingPlan(mode: plan.mode == .online ? .online : .onDevice, engine: plan.engine,
                                 uploadTo: plan.uploadTo, automatic: plan.automatic)
            },
            need: LyricsMakingNeed(a.need),
            jobs: a.jobs.map { job in
                LyricsMakingJob(id: job.id, title: job.title, artist: job.artist, uid: job.uid,
                                libraryID: job.libraryId, step: LyricsJobStep(job.stage))
            },
            models: a.models.map { model in
                SpeechModelInfo(id: model.id, name: model.name, bytes: model.bytes,
                                progress: SpeechModelProgress(model.state))
            },
            consents: a.consents.map { AudioUploadConsent(plugin: $0.plugin, origin: $0.origin) }
        )
    }
}

extension LyricsMakingNeed {
    init(_ n: AiLyricsNeed) {
        switch n {
        case .nothing: self = .nothing
        case .pro: self = .pro
        case .signIn: self = .signIn
        case .consent(let origin): self = .consent(origin: origin)
        case .model(let id, let name, let bytes): self = .model(id: id, name: name, bytes: bytes)
        case .unavailable(let reason): self = .unavailable(reason: reason)
        }
    }
}

extension LyricsJobStep {
    init(_ s: LyricsJobStage) {
        switch s {
        case .waiting: self = .waiting
        case .readingAudio: self = .readingAudio
        case .sending(let origin): self = .sending(origin: origin)
        case .recognising(let fraction): self = .recognising(fraction: fraction)
        case .timingWords: self = .timingWords
        case .done: self = .done
        case .failed(let message): self = .failed(message: message)
        case .cancelled: self = .cancelled
        }
    }
}

extension SpeechModelProgress {
    init(_ s: SpeechModelStatus) {
        switch s {
        case .notDownloaded: self = .notDownloaded
        case .downloading(let fraction): self = .downloading(fraction: fraction)
        case .ready: self = .ready
        case .failed(let message): self = .failed(message: message)
        }
    }
}

// MARK: - The look

extension LookPlace {
    init(_ p: Place) {
        switch p {
        case .home: self = .home
        case .albums: self = .albums
        case .artists: self = .artists
        case .songs: self = .songs
        case .playlists: self = .playlists
        case .favourites: self = .favourites
        case .search: self = .search
        case .sources: self = .sources
        }
    }
}

extension LookHomeSection {
    init(_ s: HomeSection) {
        switch s {
        case .summary: self = .summary
        case .recentlyAdded: self = .recentlyAdded
        case .recentlyPlayed: self = .recentlyPlayed
        case .mostPlayed: self = .mostPlayed
        case .favourites: self = .favourites
        }
    }
}

extension LookState {
    init(_ l: LookSnapshot) {
        let typeface: LookTypeface
        switch l.typeface {
        case .system: typeface = .system
        case .rounded: typeface = .rounded
        case .serif: typeface = .serif
        case .mono: typeface = .mono
        }
        let density: LookDensity
        switch l.density {
        case .compact: density = .compact
        case .standard: density = .standard
        case .roomy: density = .roomy
        }
        let weight: LookLyricsWeight
        switch l.lyricsWeight {
        case .regular: weight = .regular
        case .medium: weight = .medium
        case .semibold: weight = .semibold
        case .bold: weight = .bold
        case .heavy: weight = .heavy
        }
        let highlight: LookHighlight
        switch l.lyricsHighlight {
        case .artwork: highlight = .artwork
        case .accent: highlight = .accent
        case .custom(let rgb): highlight = .custom(rgb: rgb)
        }
        let layout: LookNowPlayingLayout
        switch l.nowPlaying {
        case .stage: layout = .stage
        case .artwork: layout = .artwork
        case .lyrics: layout = .lyrics
        }
        let mini: LookMiniPlayerStyle
        switch l.miniPlayer {
        case .bar: mini = .bar
        case .compact: mini = .compact
        case .floating: mini = .floating
        }
        self.init(version: l.version, cornerScale: l.cornerScale, glass: l.glass, aurora: l.aurora,
                  typeface: typeface, textScale: l.textScale, density: density, lyricsWeight: weight,
                  lyricsGlow: l.lyricsGlow, lyricsHighlight: highlight, sidebar: l.sidebar.map(LookPlace.init),
                  home: l.home.map(LookHomeSection.init), nowPlaying: layout, miniPlayer: mini,
                  customized: l.customized, plugin: l.plugin)
    }
}

// MARK: - Discover, details, comments, covers

extension DiscoverAnswer {
    init(_ d: DiscoverPage) {
        self.init(request: d.request, source: d.source, shelves: d.shelves.map { shelf -> DiscoverShelf in
            let layout: ShelfLayout
            switch shelf.style {
            case .row: layout = .row
            case .grid: layout = .grid
            case .chart: layout = .chart
            }
            return DiscoverShelf(id: shelf.id, title: shelf.title, layout: layout,
                                 items: shelf.items.map(SourceResult.init), more: shelf.more)
        }, error: d.error)
    }
}

extension DetailFactKey {
    init(_ k: FactKey) {
        switch k {
        case .version: self = .version
        case .genre: self = .genre
        case .language: self = .language
        case .released: self = .released
        case .label: self = .label
        case .composer: self = .composer
        case .lyricist: self = .lyricist
        case .arranger: self = .arranger
        case .producer: self = .producer
        case .popularity: self = .popularity
        case .quality: self = .quality
        case .isrc: self = .isrc
        case .disc: self = .disc
        case .track: self = .track
        case .other(let label): self = .other(label: label)
        }
    }
}

extension DetailsAnswer {
    init(_ d: DetailsPage) {
        self.init(
            request: d.request,
            item: ItemReference(d.item),
            title: d.title,
            subtitle: d.subtitle,
            notes: d.description,
            image: d.image.map(ArtworkLocation.init),
            facts: d.facts.map { DetailFact(key: DetailFactKey($0.key), value: $0.value) },
            technical: d.technical.map { t in
                TechnicalFacts(
                    format: AudioQuality(t.format),
                    container: t.container,
                    fileSizeBytes: t.fileSizeBytes,
                    durationMs: t.durationMs,
                    replayGain: t.replaygain.map {
                        ReplayGainInfo(trackGainDb: $0.trackGainDb, trackPeak: $0.trackPeak,
                                       albumGainDb: $0.albumGainDb, albumPeak: $0.albumPeak)
                    },
                    path: t.path
                )
            },
            commentCount: d.commentCount,
            attribution: d.attribution,
            error: d.error
        )
    }
}

extension SourceComment {
    init(_ c: CommentEntry) {
        self.init(id: c.id, author: c.author, avatar: c.avatar.map(ArtworkLocation.init), text: c.text,
                  likes: c.likes, timeMs: c.timeMs, replies: c.replies)
    }
}

extension CommentsAnswer {
    init(_ c: CommentsPage) {
        self.init(request: c.request, item: ItemReference(c.item), hot: c.hot.map(SourceComment.init),
                  comments: c.comments.map(SourceComment.init), total: c.total, next: c.next,
                  attribution: c.attribution, error: c.error)
    }
}

// MARK: - Snapshot, events, commands

extension CoreState {
    init(_ s: Snapshot) {
        self.init(playback: PlaybackState(s.playback), queue: QueueState(s.queue),
                  nowPlaying: s.nowPlaying.map(NowPlayingInfo.init), lyrics: s.lyrics.map(LyricsState.init),
                  theme: s.theme.map(ThemeState.init), libraryRevision: s.libraryRevision,
                  scanning: s.scanning, settings: AppSettings(s.settings), devices: DevicesState(s.devices),
                  sources: SourcesState(s.sources), muted: s.muted, remote: RemoteSession(s.remote),
                  aiLyrics: AiLyricsState(s.aiLyrics), look: LookState(s.look), pro: ProState(s.pro),
                  update: UpdateState(s.update), sourceLibrary: SourceLibraryState(s.sourceLibrary),
                  downloads: DownloadsState(s.downloads))
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
        case .aiLyrics(let snapshot): self = .aiLyrics(AiLyricsState(snapshot))
        case .look(let snapshot): self = .look(LookState(snapshot))
        case .pro(let snapshot): self = .pro(ProState(snapshot))
        case .update(let snapshot): self = .update(UpdateState(snapshot))
        case .discover(let page): self = .discover(DiscoverAnswer(page))
        case .details(let page): self = .details(DetailsAnswer(page))
        case .comments(let page): self = .comments(CommentsAnswer(page))
        case .artwork(let ready): self = .artwork(ArtworkFetched(url: ready.url, sizePx: ready.sizePx, path: ready.path))
        case .searchPage(let page): self = .searchPage(SearchPageBatch(page))
        case .sourceLibrary(let snapshot): self = .sourceLibrary(SourceLibraryState(snapshot))
        case .sourceLibraryPage(let page): self = .sourceLibraryPage(SourceLibraryPageBatch(page))
        case .downloads(let snapshot): self = .downloads(DownloadsState(snapshot))
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
        case .resetPluginSettings(let plugin): return .resetPluginSettings(plugin: plugin)
        case .makeLyrics(let song): return .makeLyrics(song: song.ffi)
        case .cancelLyrics(let job): return .cancelLyrics(job: job)
        case .deleteMadeLyrics(let song): return .deleteMadeLyrics(song: song.ffi)
        case .allowAudioUpload(let plugin, let origin): return .allowAudioUpload(plugin: plugin, origin: origin)
        case .revokeAudioUpload(let plugin, let origin): return .revokeAudioUpload(plugin: plugin, origin: origin)
        case .downloadSpeechModel(let id): return .downloadSpeechModel(id: id)
        case .cancelSpeechModelDownload(let id): return .cancelSpeechModelDownload(id: id)
        case .deleteSpeechModel(let id): return .deleteSpeechModel(id: id)
        case .activateLicence(let key): return .activateLicence(key: key)
        case .removeLicence: return .removeLicence
        case .checkForUpdates: return .checkForUpdates
        case .discover(let source): return .discover(source: source)
        case .details(let item): return .details(item: item.ffi)
        case .comments(let item, let page): return .comments(item: item.ffi, page: page)
        case .setNetworkState(let online, let metered): return .setNetworkState(online: online, metered: metered)
        case .searchMore(let plugin, let kind, let cursor):
            return .searchMore(plugin: plugin, kind: kind.ffi, cursor: cursor)
        case .refreshSourceLibrary(let plugin): return .refreshSourceLibrary(plugin: plugin)
        case .sourceLibraryMore(let plugin, let part, let cursor):
            return .sourceLibraryMore(plugin: plugin, part: part.ffi, cursor: cursor)
        case .savePlaylistCopy(let plugin, let id, let name): return .savePlaylistCopy(plugin: plugin, id: id, name: name)
        case .download(let request): return .download(target: request.ffi)
        case .cancelDownload(let id): return .cancelDownload(id: id)
        case .retryDownload(let id): return .retryDownload(id: id)
        case .removeDownload(let id): return .removeDownload(id: id)
        case .removeAllDownloads: return .removeAllDownloads
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
