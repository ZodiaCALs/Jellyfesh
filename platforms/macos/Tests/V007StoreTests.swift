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
