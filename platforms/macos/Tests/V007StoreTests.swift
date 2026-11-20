// The v0.0.7 stores on FakeCoreClient: Pro and licence keys, AI Lyrics' steps before a job, update
// checks, answers by request id, covers through the core, the distribution from the build flags
// and the new slices in an event batch.

import AppKit
import ImageIO
import UniformTypeIdentifiers
import XCTest
@testable import Jellyfesh

@MainActor
final class ProStoreTests: XCTestCase {
    func testAProActionOpensTheSheetWhileProIsInactive() {
        let store = ProStore(client: FakeCoreClient(), tracker: CommandTracker())
        XCTAssertFalse(store.require())
        XCTAssertTrue(store.sheetShown)
        var active = ProState.inactive
        active.version = 2
        active.active = true
        active.origin = .preview
        store.apply(active)
        store.sheetShown = false
        XCTAssertTrue(store.require())
        XCTAssertFalse(store.sheetShown)
    }

    func testAKeyWaitsForTheCoresAnswer() {
        let client = FakeCoreClient()
        let store = ProStore(client: client, tracker: CommandTracker())
        store.startLicenceEntry()
        store.activate(key: "  JFP1-ABCD-EFGH \n")
        XCTAssertEqual(client.sent.last, .activateLicence(key: "JFP1-ABCD-EFGH"))
        XCTAssertTrue(store.activating)
        var refused = ProState.inactive
        refused.version = 1
        refused.licenceProblem = .badSignature
        store.apply(refused)
        XCTAssertFalse(store.activating)
        XCTAssertTrue(store.licenceEntryShown, "a refused key keeps the sheet open with the reason")

        store.activate(key: "JFP1-GOOD")
        var accepted = ProState.inactive
        accepted.version = 2
        accepted.active = true
        accepted.origin = .licence(idSuffix: "ABCD", issuedAtMs: 0)
        store.apply(accepted)
        XCTAssertFalse(store.licenceEntryShown)
        XCTAssertTrue(ProText.hasLicence(store.state))
    }

    func testABlankKeyIsNotSent() {
        let client = FakeCoreClient()
        let store = ProStore(client: client, tracker: CommandTracker())
        store.activate(key: "   ")
        XCTAssertTrue(client.sent.isEmpty)
    }

    func testProIsWordedAsTheSpecSays() {
        var state = ProState.inactive
        XCTAssertEqual(ProText.status(state), String(localized: "AI Lyrics and Customize are part of Jellyfesh Pro."))
        state.origin = .licence(idSuffix: "ABCD", issuedAtMs: 0)
        XCTAssertEqual(ProText.status(state), String(localized: "Jellyfesh Pro · Licence …\("ABCD")"))
        XCTAssertEqual(ProText.problem(.unknownKey), ProText.problem(.badSignature))
        XCTAssertEqual(ProText.problem(.revoked), String(localized: "This licence key has been withdrawn."))
    }
}

@MainActor
final class AiLyricsStoreTests: XCTestCase {
    private func store(need: LyricsMakingNeed) -> (AiLyricsStore, ProStore, FakeCoreClient) {
        let client = FakeCoreClient()
        let tracker = CommandTracker()
        let store = AiLyricsStore(client: client, tracker: tracker, toasts: ToastCenter())
        var state = AiLyricsState.idle
        state.version = 1
        state.available = true
        state.plugin = "org.jellyfesh.ai-lyrics"
        state.need = need
        store.apply(state)
        return (store, ProStore(client: client, tracker: tracker), client)
    }

    func testWithoutProTheSheetOpensAndNothingIsSent() {
        let (lyrics, pro, client) = store(need: .pro)
        lyrics.make(.current, pro: pro)
        XCTAssertTrue(pro.sheetShown)
        XCTAssertTrue(client.sent.isEmpty)
    }

    func testAudioLeavesOnlyAfterTheQuestion() {
        let (lyrics, pro, client) = store(need: .consent(origin: "api.openai.com"))
        lyrics.make(.library(id: 7), pro: pro)
        XCTAssertTrue(client.sent.isEmpty, "nothing is sent before the person answers")
        let question = try? XCTUnwrap(lyrics.consentQuestion)
        XCTAssertEqual(question?.origin, "api.openai.com")
        if let question { lyrics.allow(question) }
        XCTAssertEqual(client.sent, [
            .allowAudioUpload(plugin: "org.jellyfesh.ai-lyrics", origin: "api.openai.com"),
            .makeLyrics(.library(id: 7)),
        ])
        XCTAssertNil(lyrics.consentQuestion)
    }

    func testReadyMakesTheLyrics() {
        let (lyrics, pro, client) = store(need: .nothing)
        lyrics.make(.current, pro: pro)
        XCTAssertEqual(client.sent, [.makeLyrics(.current)])
    }

    func testJobsAreFoundByTheirSong() {
        let (lyrics, _, _) = store(need: .nothing)
        var state = lyrics.state
        state.jobs = [
            LyricsMakingJob(id: 1, title: "A", artist: nil, uid: 10, libraryID: 3, step: .done),
            LyricsMakingJob(id: 2, title: "A", artist: nil, uid: 10, libraryID: 3, step: .recognising(fraction: 0.42)),
        ]
        lyrics.apply(state)
        XCTAssertEqual(lyrics.job(forEntry: 10)?.id, 2, "the newest job")
        XCTAssertEqual(lyrics.job(forLibrarySong: 3)?.id, 2)
        XCTAssertNil(lyrics.job(forEntry: nil))
        XCTAssertEqual(AiLyricsText.progress(.recognising(fraction: 0.42)) ?? 0, 0.42, accuracy: 1e-6)
        XCTAssertNil(AiLyricsText.progress(.readingAudio))
        XCTAssertNil(AiLyricsText.stage(.done))
        XCTAssertTrue(LyricsJobStep.waiting.isActive)
        XCTAssertFalse(LyricsJobStep.failed(message: "x").isActive)
    }
}

@MainActor
final class UpdatesAndAnswersTests: XCTestCase {
    func testOnlyASupportedBuildChecks() {
        let client = FakeCoreClient()
        let updates = UpdatesStore(client: client, tracker: CommandTracker())
        updates.check()
        XCTAssertTrue(client.sent.isEmpty, "store builds and builds from source have no update check")
        var state = UpdateState.unsupported
        state.supported = true
        updates.apply(state)
        updates.check()
        XCTAssertEqual(client.sent, [.checkForUpdates])
        XCTAssertNil(UpdatesStore.releaseURL(ReleaseInfo(version: "0.0.8", url: "http://example.com/r")))
        XCTAssertNotNil(UpdatesStore.releaseURL(ReleaseInfo(version: "0.0.8", url: "https://example.com/r")))
    }

    func testAnswersGoToWhoeverAsked() {
        let client = FakeCoreClient()
        let tracker = CommandTracker()
        let answers = CoreAnswers(client: client, tracker: tracker)
        var got: DiscoverAnswer?
        let id = answers.discover(source: "library", onAnswer: { got = $0 }, onFailure: { _ in XCTFail("no failure") })
        XCTAssertEqual(client.sent.last, .discover(source: "library"))
        answers.apply(discover: DiscoverAnswer(request: id + 1, source: "library", shelves: [], error: nil))
        XCTAssertNil(got, "another request's answer")
        answers.apply(discover: DiscoverAnswer(request: id, source: "library", shelves: [], error: nil))
        XCTAssertEqual(got?.request, id)

        var failure: String?
        let details = answers.details(.track(id: 4), onAnswer: { _ in XCTFail("no answer") }, onFailure: { failure = $0 })
        XCTAssertTrue(tracker.fail(details, message: "not yet"))
        XCTAssertEqual(failure, "not yet")
    }
}

@MainActor
final class SourceCoverTests: XCTestCase {
    /// A 2×2 PNG in a temporary folder.
    private func picture() throws -> String {
        let url = FileManager.default.temporaryDirectory.appendingPathComponent("jf-cover-\(UUID().uuidString).png")
        let space = CGColorSpaceCreateDeviceRGB()
        let context = try XCTUnwrap(CGContext(data: nil, width: 2, height: 2, bitsPerComponent: 8, bytesPerRow: 0,
                                              space: space, bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue))
        context.setFillColor(CGColor(red: 1, green: 0, blue: 0, alpha: 1))
        context.fill(CGRect(x: 0, y: 0, width: 2, height: 2))
        let image = try XCTUnwrap(context.makeImage())
        let destination = try XCTUnwrap(CGImageDestinationCreateWithURL(url as CFURL, UTType.png.identifier as CFString,
                                                                        1, nil))
        CGImageDestinationAddImage(destination, image, nil)
        XCTAssertTrue(CGImageDestinationFinalize(destination))
        return url.path
    }

    func testACachedCoverLoadsAtOnce() async throws {
        let client = FakeCoreClient()
        let url = "https://img.example.com/cached.jpg"
        client.cachedSourceArtwork[url] = try picture()
        let loader = ArtworkLoader(client: client)
        let image = await loader.image(.url(url), pixels: 60)
        XCTAssertNotNil(image)
        XCTAssertEqual(client.sourceArtworkRequests.first?.sizePx, 64, "the size the core makes")
    }

    func testACoverTheCoreFetchesArrivesWithItsEvent() async throws {
        let client = FakeCoreClient()
        let url = "https://img.example.com/fetched.jpg"
        let path = try picture()
        let loader = ArtworkLoader(client: client)
        let load = Task { await loader.image(.url(url), pixels: 128) }
        await eventually { !client.sourceArtworkRequests.isEmpty }
        loader.artworkFetched(ArtworkFetched(url: url, sizePx: 128, path: path))
        let image = await load.value
        XCTAssertNotNil(image)
    }

    func testACoverThatCannotBeHadKeepsThePlaceholder() async {
        let client = FakeCoreClient()
        let url = "https://img.example.com/gone.jpg"
        let loader = ArtworkLoader(client: client)
        let load = Task { await loader.image(.url(url), pixels: 64) }
        await eventually { !client.sourceArtworkRequests.isEmpty }
        loader.artworkFetched(ArtworkFetched(url: url, sizePx: 64, path: nil))
        let image = await load.value
        XCTAssertNil(image)
        let again = await loader.image(.url(url), pixels: 64)
        XCTAssertNil(again)
        XCTAssertEqual(client.sourceArtworkRequests.count, 1, "not asked for again")
    }
}

final class BuildFlagTests: XCTestCase {
    func testTheDistributionComesFromTheBuildFlags() {
        XCTAssertEqual(AppDistribution.from(infoValue: "direct"), .direct)
        XCTAssertEqual(AppDistribution.from(infoValue: "app-store"), .appStore)
        XCTAssertEqual(AppDistribution.from(infoValue: "source"), .source)
        XCTAssertEqual(AppDistribution.from(infoValue: "$(JF_DISTRIBUTION)"), .source)
        XCTAssertEqual(AppDistribution.from(infoValue: nil), .source)
        XCTAssertTrue(AppDistribution.appStore.isAppStore)
    }

    func testTheNewSlicesCoalesceAndAnswersAreKept() {
        var older = LookState.luminous
        older.version = 1
        var newer = LookState.luminous
        newer.version = 2
        let answer = DiscoverAnswer(request: 5, source: "library", shelves: [], error: nil)
        let batch: [CoreEvent] = [.look(older), .discover(answer), .look(newer),
                                  .artwork(ArtworkFetched(url: "u", sizePx: 64, path: nil))]
        XCTAssertEqual(EventBatch.coalesced(batch), [.discover(answer), .look(newer),
                                                    .artwork(ArtworkFetched(url: "u", sizePx: 64, path: nil))])
    }
}

@MainActor
final class ExtensionsStoreTests: XCTestCase {
    private func extensionPlugin(_ id: String, version: String = "0.0.7") -> SourcePlugin {
        var plugin = Fixture.plugin(id, methods: [], schema: #"{"type":"object","properties":{"glass":{"type":"number"}}}"#)
        plugin.isExtension = true
        plugin.requiresPro = true
        plugin.version = version
        return plugin
    }

    func testExtensionsAreListedApartFromSources() {
        let store = SourcesStore(client: FakeCoreClient(), tracker: CommandTracker(), toasts: ToastCenter())
        let customize = extensionPlugin(LookStore.customizeID)
        var state = SourcesState(version: 1, plugins: [Fixture.plugin(), customize], allowCommunity: false)
        state.canInstallPlugins = false
        store.apply(state)
        XCTAssertEqual(store.plugins.map(\.id), ["org.example.subsonic"])
        XCTAssertEqual(store.extensions.map(\.id), [LookStore.customizeID])
        XCTAssertEqual(store.plugin(LookStore.customizeID)?.id, LookStore.customizeID, "settings still find it")
        XCTAssertFalse(store.canInstallPlugins)
    }

    func testSettingsShowInTheAppsLanguage() async {
        XCTAssertNil(SourcesStore.schemaLanguage(preferred: ["en"]))
        XCTAssertNil(SourcesStore.schemaLanguage(preferred: ["en-GB", "ja"]))
        XCTAssertEqual(SourcesStore.schemaLanguage(preferred: ["zh-Hans"]), "zh-Hans")
        XCTAssertNil(SourcesStore.schemaLanguage(preferred: []))

        let client = FakeCoreClient()
        let customize = extensionPlugin(LookStore.customizeID)
        let localized = #"{"type":"object","properties":{"glass":{"type":"number","title":"玻璃"}}}"#
        client.localizedSchemas["\(LookStore.customizeID)|zh-Hans"] = localized
        let store = SourcesStore(client: client, tracker: CommandTracker(), toasts: ToastCenter())
        XCTAssertEqual(store.settingsSchema(of: customize), customize.settingsSchemaJSON, "English until asked")
        await store.loadSettingsSchema(of: customize, language: nil)
        XCTAssertEqual(store.settingsSchema(of: customize), customize.settingsSchemaJSON)
        await store.loadSettingsSchema(of: customize, language: "zh-Hans")
        XCTAssertEqual(store.settingsSchema(of: customize), localized)
        let updated = extensionPlugin(LookStore.customizeID, version: "0.0.8")
        XCTAssertEqual(store.settingsSchema(of: updated), updated.settingsSchemaJSON, "a new version asks again")
    }

    func testResetToDefaults() {
        let client = FakeCoreClient()
        let store = SourcesStore(client: client, tracker: CommandTracker(), toasts: ToastCenter())
        store.resetSettings(LookStore.customizeID)
        XCTAssertEqual(client.sent.last, .resetPluginSettings(plugin: LookStore.customizeID))
    }
}

/// The App Store, as calls and canned answers.
@MainActor
private final class FakePurchasing: ProPurchasing {
    var priceText: String? = "$9.99"
    var owned = ProEntitlement(store: .appStore, product: "org.jellyfesh.pro", active: false, transaction: nil)
    var nextPurchase: Result<ProEntitlement?, Error> = .success(nil)
    var syncFailure: Error?
    private(set) var syncs = 0
    private(set) var onChange: (@MainActor (ProEntitlement) -> Void)?

    func price() async -> String? { priceText }
    func purchase() async throws -> ProEntitlement? { try nextPurchase.get() }
    func currentEntitlement() async -> ProEntitlement { owned }

    func sync() async throws {
        syncs += 1
        if let syncFailure { throw syncFailure }
    }

    func listen(_ onChange: @escaping @MainActor (ProEntitlement) -> Void) {
        self.onChange = onChange
    }
}

@MainActor
final class ProPurchaseTests: XCTestCase {
    private let bought = ProEntitlement(store: .appStore, product: "org.jellyfesh.pro", active: true, transaction: "2000")

    private func appStoreState() -> ProState {
        var state = ProState.inactive
        state.version = 1
        state.stores = [.appStore]
        state.licenceKeys = false
        return state
    }

    func testTheAppStoreSaysWhatThePersonOwnsAtStartAndAfter() async {
        let client = FakeCoreClient()
        let store = ProStore(client: client, tracker: CommandTracker())
        let purchasing = FakePurchasing()
        store.apply(appStoreState())
        XCTAssertFalse(store.sellsInAppStore, "not before the App Store is reached")
        await store.attach(purchasing)
        XCTAssertTrue(store.sellsInAppStore)
        XCTAssertEqual(store.price, "$9.99")
        await eventually { client.storeEntitlements.count == 1 }
        XCTAssertEqual(client.storeEntitlements.first?.active, false)
        purchasing.onChange?(bought)
        await eventually { client.storeEntitlements.count == 2 }
        XCTAssertEqual(client.storeEntitlements.last, bought)
    }

    func testBuyingReportsThePurchaseAndClosesTheSheet() async {
        let client = FakeCoreClient()
        let store = ProStore(client: client, tracker: CommandTracker())
        let purchasing = FakePurchasing()
        store.apply(appStoreState())
        await store.attach(purchasing)
        await eventually { client.storeEntitlements.count == 1 }
        XCTAssertFalse(store.require())

        purchasing.nextPurchase = .success(nil)
        store.buy()
        await eventually { !store.buying }
        XCTAssertTrue(store.sheetShown, "cancelled: the sheet stays")
        XCTAssertEqual(client.storeEntitlements.count, 1)

        purchasing.nextPurchase = .failure(PurchaseFailure.unverified)
        store.buy()
        await eventually { !store.buying }
        XCTAssertEqual(store.storeFailure, PurchaseFailure.unverified.errorDescription)

        purchasing.nextPurchase = .success(bought)
        store.buy()
        await eventually { !store.buying }
        XCTAssertFalse(store.sheetShown)
        XCTAssertNil(store.storeFailure)
        await eventually { client.storeEntitlements.count == 2 }
        XCTAssertEqual(client.storeEntitlements.last, bought)
    }

    func testRestorePurchasesSyncsThenReports() async {
        let client = FakeCoreClient()
        let store = ProStore(client: client, tracker: CommandTracker())
        let purchasing = FakePurchasing()
        store.apply(appStoreState())
        await store.attach(purchasing)
        await eventually { client.storeEntitlements.count == 1 }
        purchasing.owned = bought
        store.restorePurchases()
        await eventually { !store.buying }
        XCTAssertEqual(purchasing.syncs, 1)
        await eventually { client.storeEntitlements.count == 2 }
        XCTAssertEqual(client.storeEntitlements.last, bought)

        purchasing.syncFailure = PurchaseFailure.unavailable
        store.restorePurchases()
        await eventually { !store.buying }
        XCTAssertEqual(store.storeFailure, PurchaseFailure.unavailable.errorDescription)
    }
}

final class LyricsMakingStepTests: XCTestCase {
    private func state(_ need: LyricsMakingNeed, available: Bool = true,
                       models: [SpeechModelInfo] = []) -> AiLyricsState {
        var state = AiLyricsState.idle
        state.version = 1
        state.available = available
        state.plugin = "org.jellyfesh.ai-lyrics"
        state.need = need
        state.models = models
        return state
    }

    private func job(_ step: LyricsJobStep) -> LyricsMakingJob {
        LyricsMakingJob(id: 9, title: "Song", artist: nil, uid: 3, libraryID: 7, step: step)
    }

    func testTheSongsJobComesFirst() {
        let running = AiLyricsText.step(job: job(.recognising(fraction: 0.42)), state: state(.nothing))
        guard case .running(let id, _, let progress) = running else { return XCTFail("a running job") }
        XCTAssertEqual(id, 9)
        XCTAssertEqual(progress ?? 0, 0.42, accuracy: 1e-6)
        if case .running(_, _, let none) = AiLyricsText.step(job: job(.readingAudio), state: state(.nothing)) {
            XCTAssertNil(none, "reading audio has no measure")
        } else {
            XCTFail("a running job")
        }
        XCTAssertEqual(AiLyricsText.step(job: job(.failed(message: "No speech")), state: state(.nothing)),
                       .failed(message: "No speech"))
        XCTAssertTrue(AiLyricsText.step(job: job(.waiting), state: state(.nothing, available: false)).isJob,
                      "a job shows even after AI Lyrics went off")
        XCTAssertEqual(AiLyricsText.step(job: job(.done), state: state(.nothing)), .create)
        XCTAssertEqual(AiLyricsText.step(job: job(.cancelled), state: state(.nothing)), .create)
    }

    func testWhatMakingLyricsNeeds() {
        XCTAssertEqual(AiLyricsText.step(job: nil, state: state(.nothing, available: false)), .none)
        XCTAssertEqual(AiLyricsText.step(job: nil, state: state(.pro)), .create, "the Pro sheet comes when chosen")
        XCTAssertEqual(AiLyricsText.step(job: nil, state: state(.consent(origin: "api.groq.com"))), .create)
        XCTAssertEqual(AiLyricsText.step(job: nil, state: state(.signIn)), .signIn)
        XCTAssertEqual(AiLyricsText.step(job: nil, state: state(.unavailable(reason: " "))), .none)
        XCTAssertEqual(AiLyricsText.step(job: nil, state: state(.unavailable(reason: "No recogniser"))),
                       .unavailable(reason: "No recogniser"))
        let need = LyricsMakingNeed.model(id: "base", name: "Base", bytes: 147_951_465)
        guard case .downloadModel(let id, _) = AiLyricsText.step(job: nil, state: state(need)) else {
            return XCTFail("download first")
        }
        XCTAssertEqual(id, "base")
        let downloading = SpeechModelInfo(id: "base", name: "Base", bytes: 147_951_465,
                                          progress: .downloading(fraction: 0.35))
        guard case .downloadingModel(_, _, let fraction) = AiLyricsText.step(job: nil, state: state(need, models: [downloading]))
        else { return XCTFail("downloading") }
        XCTAssertEqual(fraction, 0.35, accuracy: 1e-6)
    }

    @MainActor
    func testMadeLyricsSayWhoMadeThem() {
        let lyrics = LyricsStore()
        let document = LyricsDocument(language: "en", timing: .word, lines: [], tracks: [], format: "made", origin: nil)
        lyrics.apply(LyricsState(uid: 3, document: document, timeline: nil,
                                 origin: .made(plugin: "org.jellyfesh.ai-lyrics", engine: "Whisper Base")))
        XCTAssertEqual(lyrics.madeBy(for: 3), "Whisper Base")
        XCTAssertNil(lyrics.madeBy(for: 4), "another song's")
        lyrics.apply(LyricsState(uid: 3, document: document, timeline: nil, origin: .embedded))
        XCTAssertNil(lyrics.madeBy(for: 3))
    }
}

@MainActor
final class DiscoverTests: XCTestCase {
    private func shelf(_ id: String, items: [SourceResult], more: String? = nil) -> DiscoverShelf {
        DiscoverShelf(id: id, title: "Shelf \(id)", layout: .row, items: items, more: more)
    }

    func testThePageShowsTheCoresShelves() {
        let client = FakeCoreClient()
        let answers = CoreAnswers(client: client, tracker: CommandTracker())
        let page = DiscoverModel(source: "org.jellyfesh.jellyfin")
        page.load(answers)
        XCTAssertTrue(page.loading)
        XCTAssertFalse(page.loaded)
        guard case .discover(let source)? = client.sent.last else { return XCTFail("asked for the page") }
        XCTAssertEqual(source, "org.jellyfesh.jellyfin")
        let items = [Fixture.result("a", source: source), Fixture.result("b", source: source)]
        answers.apply(discover: DiscoverAnswer(request: client.lastCommandID, source: source,
                                               shelves: [shelf("for_you", items: items), shelf("empty", items: [])],
                                               error: nil))
        XCTAssertTrue(page.loaded)
        XCTAssertFalse(page.loading)
        XCTAssertEqual(page.shelves.map(\.id), ["for_you"], "an empty shelf is left out")
        XCTAssertNil(page.error)
    }

    func testAFailureCanBeTriedAgain() {
        let client = FakeCoreClient()
        let tracker = CommandTracker()
        let answers = CoreAnswers(client: client, tracker: tracker)
        let page = DiscoverModel(source: "org.jellyfesh.subsonic")
        page.load(answers)
        XCTAssertTrue(tracker.fail(client.lastCommandID, message: "The server did not answer."))
        XCTAssertEqual(page.error, "The server did not answer.")
        XCTAssertFalse(page.loading)
        page.load(answers)
        XCTAssertNil(page.error)
        answers.apply(discover: DiscoverAnswer(request: client.lastCommandID, source: "org.jellyfesh.subsonic",
                                               shelves: [], error: "Sign in first."))
        XCTAssertEqual(page.error, "Sign in first.")
        XCTAssertTrue(page.loaded)
    }

    func testShelfWordsAndPlaying() {
        let source = "org.jellyfesh.jellyfin"
        XCTAssertEqual(DiscoverText.title(shelf("browse", items: []), sourceName: "Jellyfin"), "Jellyfin")
        XCTAssertEqual(DiscoverText.title(shelf("node-17", items: []), sourceName: "Jellyfin"), "Shelf node-17")
        XCTAssertFalse(DiscoverText.title(shelf("for_you", items: []), sourceName: "Jellyfin").isEmpty)

        var unplayable = Fixture.result("x", source: source, track: .source(SourceSong(plugin: source, id: "x", title: "x")))
        unplayable.playable = false
        let album = Fixture.result("al", source: source, kind: .album)
        let first = Fixture.result("1", source: source, track: .source(SourceSong(plugin: source, id: "1", title: "1")))
        let second = Fixture.result("2", source: source, track: .source(SourceSong(plugin: source, id: "2", title: "2")))
        let row = shelf("mix", items: [first, unplayable, album, second])
        let queue = DiscoverText.queue(of: row, from: second)
        XCTAssertEqual(queue?.tracks, [first.track, second.track].compactMap { $0 })
        XCTAssertEqual(queue?.start, 1)
        XCTAssertNil(DiscoverText.queue(of: row, from: unplayable))
        XCTAssertNil(DiscoverText.queue(of: row, from: album))
    }
}

final class SourceArtistAndAlbumTests: XCTestCase {
    func testASourceSongKnowsItsArtistAndAlbum() {
        var song = Fixture.result("t1", source: "org.jellyfesh.subsonic",
                                  track: .source(SourceSong(plugin: "org.jellyfesh.subsonic", id: "t1", title: "Song")))
        song.subtitle = "Artist"
        song.album = "Album"
        song.artistID = "ar-1"
        song.albumID = "al-1"
        let row = TrackItem(result: song, artwork: nil)
        XCTAssertEqual(row.sourceArtistID, "ar-1")
        XCTAssertEqual(row.sourceAlbumID, "al-1")
        XCTAssertEqual(row.source, "org.jellyfesh.subsonic")
    }

    func testTheirPagesAreTheSourcesAlbumAndArtist() {
        let album = SourcePage.album(plugin: "org.jellyfesh.subsonic", id: "al-1", title: "Album", artist: "Artist")
        XCTAssertEqual(album.kind, .album)
        XCTAssertEqual(album.itemID, "al-1")
        XCTAssertEqual(album.subtitle, "Artist")
        XCTAssertTrue(album.playable, "Play and Shuffle play Collection::Source")
        XCTAssertEqual(album.itemReference, .source(plugin: "org.jellyfesh.subsonic", kind: .album, id: "al-1"))
        let artist = SourcePage.artist(plugin: "org.jellyfesh.subsonic", id: "ar-1", name: "Artist")
        XCTAssertEqual(artist.kind, .artist)
        XCTAssertEqual(artist.title, "Artist")
        XCTAssertFalse(SourcePage.album(plugin: "p", id: "x", title: "", artist: nil).title.isEmpty)
    }
}
