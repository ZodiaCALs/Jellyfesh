import AppKit
import XCTest
@testable import Jellyfesh

private let notAvailable = "online sources are not available in this build yet"

@MainActor
final class SearchStoreTests: XCTestCase {
    func testKeepsTheLatestSearchAndPutsTheLibraryFirst() {
        let client = FakeCoreClient()
        let store = SearchStore(client: client, tracker: CommandTracker())
        store.search("tide")
        let request = client.lastCommandID
        XCTAssertEqual(client.sent.last, .search(query: "tide", area: .all))
        XCTAssertTrue(store.isWaiting)

        store.apply(SearchBatch(request: request - 1, query: "old", source: SearchStore.library,
                                items: [Fixture.result("1", source: SearchStore.library)], error: nil, done: true))
        XCTAssertTrue(store.sections.isEmpty, "answers to an older search are ignored")

        store.apply(SearchBatch(request: request, query: "tide", source: "org.example.subsonic",
                                items: [Fixture.result("a", source: "org.example.subsonic")], error: nil, done: false))
        store.apply(SearchBatch(request: request, query: "tide", source: SearchStore.library,
                                items: [Fixture.result("b", source: SearchStore.library, kind: .album)], error: nil,
                                done: true))
        XCTAssertEqual(store.sections.map(\.source), [SearchStore.library, "org.example.subsonic"])
        XCTAssertTrue(store.done)
        XCTAssertFalse(store.isWaiting)
        XCTAssertFalse(store.foundNothing)
    }

    func testTabsShowMoreAndTheSourcesSearchedApart() {
        let client = FakeCoreClient()
        let store = SearchStore(client: client, tracker: CommandTracker())
        store.search("tide")
        let request = client.lastCommandID
        let song = Fixture.result("s1", source: "org.example.subsonic")
        let spotify = Fixture.result("sp1", source: "org.example.spotify")
        store.apply(SearchBatch(request: request, query: "tide", source: SearchStore.library,
                                items: [Fixture.result("1", source: SearchStore.library)], error: nil, done: false))
        store.apply(SearchBatch(request: request, query: "tide", source: "org.example.subsonic", items: [song],
                                error: nil, done: false, merged: true,
                                groups: [SearchKindGroup(kind: .track, items: [song], next: "c2", total: 40)]))
        store.apply(SearchBatch(request: request, query: "tide", source: "org.example.spotify", items: [spotify],
                                error: nil, done: true, merged: false,
                                groups: [SearchKindGroup(kind: .track, items: [spotify], next: nil, total: 1)]))
        XCTAssertEqual(store.tabs, [.all, .library, .source("org.example.subsonic"), .source("org.example.spotify")])
        XCTAssertEqual(store.sourceSections.map(\.source), ["org.example.subsonic"], "From Your Sources merges only")
        XCTAssertEqual(store.separateSections.map(\.source), ["org.example.spotify"], "linked from All")

        store.showMore(source: "org.example.subsonic", kind: .track)
        XCTAssertEqual(client.sent.last, .searchMore(plugin: "org.example.subsonic", kind: .track, cursor: "c2"))
        XCTAssertTrue(store.loadingMore.contains(SearchStore.moreKey("org.example.subsonic", .track)))
        let more = Fixture.result("s2", source: "org.example.subsonic")
        store.apply(page: SearchPageBatch(request: client.lastCommandID, query: "tide", source: "org.example.subsonic",
                                          kind: .track, items: [song, more], next: nil, error: nil))
        let group = store.section(for: "org.example.subsonic")?.groups.first
        XCTAssertEqual(group?.items.map(\.itemID), ["s1", "s2"], "new rows join their group once")
        XCTAssertNil(group?.next, "no more to show")
        XCTAssertTrue(store.loadingMore.isEmpty)

        store.tab = .source("org.example.spotify")
        store.search("harbour")
        XCTAssertEqual(store.tab, .all, "a new search starts on All")
    }

    func testARepeatedSourceReplacesItsSection() {
        let client = FakeCoreClient()
        let store = SearchStore(client: client, tracker: CommandTracker())
        store.search("x")
        let request = client.lastCommandID
        store.apply(SearchBatch(request: request, query: "x", source: "p", items: [Fixture.result("1", source: "p")],
                                error: nil, done: false))
        store.apply(SearchBatch(request: request, query: "x", source: "p", items: [], error: nil, done: true))
        XCTAssertEqual(store.sections.count, 1)
        XCTAssertTrue(store.foundNothing)
        store.clear()
        XCTAssertTrue(store.sections.isEmpty)
        XCTAssertFalse(store.isWaiting)
    }

    func testGroupsByKindSongsFirst() {
        let section = SearchSection(source: SearchStore.library, items: [
            Fixture.result("c", source: SearchStore.library, kind: .artist),
            Fixture.result("a", source: SearchStore.library, kind: .track),
            Fixture.result("b", source: SearchStore.library, kind: .track),
        ], error: nil)
        XCTAssertEqual(section.kinds, [.track, .artist])
        XCTAssertEqual(section.items(of: .track).map(\.itemID), ["a", "b"])
    }

    func testFindsLibraryArtworkWithOneQuery() async {
        let client = FakeCoreClient()
        client.librarySearchResult = LibrarySearch(songs: [Fixture.song(7, artwork: "abc")], albums: [], artists: [])
        let store = SearchStore(client: client, tracker: CommandTracker())
        store.search("seven")
        let item = Fixture.result("7", source: SearchStore.library, track: .library(id: 7))
        store.apply(SearchBatch(request: client.lastCommandID, query: "seven", source: SearchStore.library,
                                items: [item], error: nil, done: true))
        await eventually { store.artwork(for: item) == .hash("abc") }
        XCTAssertEqual(client.librarySearches, ["seven"])
    }

    func testFallsBackToTheLibraryWhenTheCommandFails() async {
        let client = FakeCoreClient()
        client.librarySearchResult = LibrarySearch(songs: [Fixture.song(1), Fixture.song(2)], albums: [], artists: [])
        let tracker = CommandTracker()
        let store = SearchStore(client: client, tracker: tracker)
        store.search("anything")
        XCTAssertTrue(tracker.fail(client.lastCommandID, message: notAvailable))
        await eventually { store.done }
        XCTAssertEqual(store.sections.first?.source, SearchStore.library)
        XCTAssertEqual(store.sections.first?.items.compactMap(\.track), [.library(id: 1), .library(id: 2)])
    }

    func testTheLibraryComesFirstThenEachSourceThatFoundSomething() {
        let client = FakeCoreClient()
        let store = SearchStore(client: client, tracker: CommandTracker())
        store.search("tide")
        let request = client.lastCommandID
        store.apply(SearchBatch(request: request, query: "tide", source: "org.example.empty", items: [], error: nil,
                                done: false))
        store.apply(SearchBatch(request: request, query: "tide", source: "org.example.subsonic",
                                items: [Fixture.result("a", source: "org.example.subsonic")], error: nil, done: false))
        store.apply(SearchBatch(request: request, query: "tide", source: "org.example.down", items: [],
                                error: "The server did not answer.", done: false))
        XCTAssertNil(store.librarySection, "the library found nothing yet")
        store.apply(SearchBatch(request: request, query: "tide", source: SearchStore.library,
                                items: [Fixture.result("b", source: SearchStore.library)], error: nil, done: true))
        XCTAssertEqual(store.librarySection?.source, SearchStore.library)
        // From Your Sources: in the order they answered, without the ones that found nothing.
        XCTAssertEqual(store.sourceSections.map(\.source), ["org.example.subsonic", "org.example.down"])
    }

    func testTypingWaitsForAPause() async {
        let client = FakeCoreClient()
        let store = SearchStore(client: client, tracker: CommandTracker())
        store.text = "t"
        store.textChanged()
        store.text = "ti"
        store.textChanged()
        store.text = "tide"
        store.textChanged()
        XCTAssertTrue(client.sent.isEmpty)
        await eventually { !client.sent.isEmpty }
        XCTAssertEqual(client.sent, [.search(query: "tide", area: .all)])
    }
}

@MainActor
final class SourcesStoreTests: XCTestCase {
    func testASignInFlowFollowsTheCoresSteps() {
        let client = FakeCoreClient()
        let store = SourcesStore(client: client, tracker: CommandTracker(), toasts: ToastCenter())
        let plugin = Fixture.plugin(methods: [.qr])
        store.beginSignIn(plugin, method: .qr)
        XCTAssertEqual(client.sent.last, .beginLogin(plugin: plugin.id, method: .qr))
        XCTAssertNil(store.signIn?.step)

        store.apply(signIn: SignInUpdate(plugin: plugin.id, flow: 9, step: .showQR(data: "qr", expiresAtMs: nil)))
        XCTAssertEqual(store.signIn?.flow, 9)
        XCTAssertEqual(store.signIn?.step, .showQR(data: "qr", expiresAtMs: nil))

        store.apply(signIn: SignInUpdate(plugin: plugin.id, flow: 10, step: .failed("other flow")))
        XCTAssertEqual(store.signIn?.step, .showQR(data: "qr", expiresAtMs: nil), "another flow's step is ignored")
        store.apply(signIn: SignInUpdate(plugin: "org.example.other", flow: 9, step: .failed("other plugin")))
        XCTAssertEqual(store.signIn?.flow, 9)

        store.endSignIn()
        XCTAssertEqual(client.sent.last, .cancelLogin(plugin: plugin.id, flow: 9))
        XCTAssertNil(store.signIn)
    }

    func testAFinishedFlowIsNotCancelled() {
        let client = FakeCoreClient()
        let store = SourcesStore(client: client, tracker: CommandTracker(), toasts: ToastCenter())
        let plugin = Fixture.plugin()
        store.beginSignIn(plugin, method: .passwordExchange)
        store.apply(signIn: SignInUpdate(plugin: plugin.id, flow: 3,
                                         step: .askFields([SignInField(name: "user", label: "User", secret: false,
                                                                       placeholder: nil)])))
        store.submitSignIn(fields: ["user": "ada"])
        XCTAssertEqual(client.sent.last, .submitLogin(plugin: plugin.id, flow: 3, fields: ["user": "ada"]))
        store.apply(signIn: SignInUpdate(plugin: plugin.id, flow: 3, step: .done(account: "ada")))
        XCTAssertTrue(store.signIn?.isOver == true)
        let count = client.sent.count
        store.endSignIn()
        XCTAssertEqual(client.sent.count, count)
    }

    func testNotAvailableIsANoticeNotAnError() {
        let client = FakeCoreClient()
        let tracker = CommandTracker()
        let toasts = ToastCenter()
        let store = SourcesStore(client: client, tracker: tracker, toasts: toasts)
        store.install(path: "/tmp/org.jellyfesh.subsonic.jfpkg")
        XCTAssertTrue(tracker.fail(client.lastCommandID, message: notAvailable))
        XCTAssertTrue(store.unavailable)
        XCTAssertNil(toasts.message)

        store.uninstall("org.example.subsonic")
        XCTAssertTrue(tracker.fail(client.lastCommandID, message: "the plugin is in use"))
        XCTAssertEqual(toasts.message, "the plugin is in use")
    }

    func testSignInThatIsNotAvailableEndsTheFlowCalmly() {
        let client = FakeCoreClient()
        let tracker = CommandTracker()
        let store = SourcesStore(client: client, tracker: tracker, toasts: ToastCenter())
        store.beginSignIn(Fixture.plugin(), method: .apiKey)
        XCTAssertTrue(tracker.fail(client.lastCommandID, message: notAvailable))
        XCTAssertEqual(store.signIn?.unavailable, true)
        XCTAssertTrue(store.signIn?.isOver == true)
    }

    func testEnablingACommunityPluginAcknowledgesItFirst() {
        let client = FakeCoreClient()
        let store = SourcesStore(client: client, tracker: CommandTracker(), toasts: ToastCenter())
        store.acknowledgeAndEnable("org.example.netease")
        XCTAssertEqual(client.sent, [
            .acknowledgeUnofficial(id: "org.example.netease"),
            .setPluginEnabled(id: "org.example.netease", enabled: true),
        ])
    }

    func testBrowsePagesGoToTheirPage() {
        let client = FakeCoreClient()
        let store = SourcesStore(client: client, tracker: CommandTracker(), toasts: ToastCenter())
        let model = BrowseModel(plugin: "p", node: nil)
        model.start(store)
        XCTAssertEqual(client.sent.last, .browse(plugin: "p", node: nil, page: 0))
        XCTAssertTrue(model.isLoading)
        let request = client.lastCommandID
        let tracks = [Fixture.result("1", source: "p", track: .library(id: 1)),
                      Fixture.result("2", source: "p", kind: .folder),
                      Fixture.result("3", source: "p", track: .library(id: 3))]
        store.apply(browse: BrowseBatch(request: request + 1, plugin: "p", node: nil, items: tracks, nextPage: nil,
                                        error: nil))
        XCTAssertTrue(model.items.isEmpty, "another page's answer")
        store.apply(browse: BrowseBatch(request: request, plugin: "p", node: nil, items: tracks, nextPage: 1,
                                        error: nil))
        XCTAssertEqual(model.items.count, 3)
        XCTAssertEqual(model.nextPage, 1)
        XCTAssertFalse(model.isLoading)
        let queue = model.tracks(startingAt: tracks[2])
        XCTAssertEqual(queue?.tracks, [.library(id: 1), .library(id: 3)])
        XCTAssertEqual(queue?.start, 1)
    }

    func testAnAddressStillInItsFieldIsSentBeforeSignIn() {
        let client = FakeCoreClient()
        let store = SourcesStore(client: client, tracker: CommandTracker(), toasts: ToastCenter())
        let plugin = Fixture.plugin(settings: #"{"quality":"app"}"#)
        store.apply(SourcesState(version: 1, plugins: [plugin], allowCommunity: false))
        // Typed, but neither Return nor a click elsewhere yet.
        store.editSetting(plugin.id, key: "server_url", value: .string("https://music.example"))
        XCTAssertTrue(client.sent.isEmpty)
        store.beginSignIn(plugin, method: .passwordExchange)
        XCTAssertEqual(client.sent, [
            .setPluginSettings(id: plugin.id, json: #"{"quality":"app","server_url":"https://music.example"}"#),
            .beginLogin(plugin: plugin.id, method: .passwordExchange),
        ])
    }

    func testUnchangedSettingsAreNotSentAgain() {
        let client = FakeCoreClient()
        let store = SourcesStore(client: client, tracker: CommandTracker(), toasts: ToastCenter())
        let plugin = Fixture.plugin(settings: #"{"server_url":"https://music.example"}"#)
        store.apply(SourcesState(version: 1, plugins: [plugin], allowCommunity: false))
        store.editSetting(plugin.id, key: "server_url", value: .string("https://music.example"))
        store.commitSettings(plugin.id)
        store.commitSettings(plugin.id)
        XCTAssertTrue(client.sent.isEmpty)
    }

    func testQuickChangesBuildOnEachOther() {
        let client = FakeCoreClient()
        let store = SourcesStore(client: client, tracker: CommandTracker(), toasts: ToastCenter())
        let plugin = Fixture.plugin(settings: "{}")
        store.apply(SourcesState(version: 1, plugins: [plugin], allowCommunity: false))
        store.setSetting(plugin.id, key: "quality", value: .string("high"))
        // The core has not reported the first change back yet.
        store.setSetting(plugin.id, key: "transcode_format", value: .string("opus"))
        XCTAssertEqual(client.sent.last,
                       .setPluginSettings(id: plugin.id, json: #"{"quality":"high","transcode_format":"opus"}"#))
    }

    func testConnectingToAServiceSignsInThroughItsPlugin() {
        let client = FakeCoreClient()
        let store = SourcesStore(client: client, tracker: CommandTracker(), toasts: ToastCenter())
        let preset = Fixture.preset()
        store.apply(SourcesState(version: 1, plugins: [], presets: [preset], allowCommunity: false))
        XCTAssertEqual(store.presets, [preset])
        store.connect(preset, address: "  http://192.168.1.20:4533\n", method: .passwordExchange)
        XCTAssertEqual(client.sent, [
            .connectSource(preset: "navidrome", address: "http://192.168.1.20:4533", method: .passwordExchange),
        ])
        XCTAssertEqual(store.signIn?.plugin, "org.jellyfesh.subsonic")
        XCTAssertEqual(store.signIn?.pluginName, "Navidrome")
        XCTAssertNil(store.signIn?.step)
        // The steps arrive for the plugin, as after BeginLogin.
        store.apply(signIn: SignInUpdate(plugin: "org.jellyfesh.subsonic", flow: 4, step: .waiting("Connecting")))
        XCTAssertEqual(store.signIn?.flow, 4)
    }

    func testAServerNeedsItsAddress() {
        let client = FakeCoreClient()
        let store = SourcesStore(client: client, tracker: CommandTracker(), toasts: ToastCenter())
        let preset = Fixture.preset()
        XCTAssertFalse(store.canConnect(preset, address: " \n"))
        store.connect(preset, address: " \n", method: .passwordExchange)
        XCTAssertTrue(client.sent.isEmpty)
        XCTAssertNil(store.signIn)
        // A service that is not on a server of the person's own is sent no address.
        let service = Fixture.preset("example", needsAddress: false)
        XCTAssertTrue(store.canConnect(service, address: ""))
        store.connect(service, address: "ignored", method: .oauth)
        XCTAssertEqual(client.sent, [.connectSource(preset: "example", address: nil, method: .oauth)])
    }

    func testConnectingCancelsTheFlowStillRunning() {
        let client = FakeCoreClient()
        let store = SourcesStore(client: client, tracker: CommandTracker(), toasts: ToastCenter())
        let plugin = Fixture.plugin(methods: [.qr])
        store.beginSignIn(plugin, method: .qr)
        store.apply(signIn: SignInUpdate(plugin: plugin.id, flow: 9, step: .showQR(data: "qr", expiresAtMs: nil)))
        store.connect(Fixture.preset(), address: "http://192.168.1.20:4533", method: .passwordExchange)
        XCTAssertEqual(client.sent, [
            .beginLogin(plugin: plugin.id, method: .qr),
            .cancelLogin(plugin: plugin.id, flow: 9),
            .connectSource(preset: "navidrome", address: "http://192.168.1.20:4533", method: .passwordExchange),
        ])
        XCTAssertEqual(store.signIn?.plugin, "org.jellyfesh.subsonic")
        XCTAssertNil(store.signIn?.flow, "the new flow waits for its own first step")
    }

    func testSettingsStillBeingTypedGoBeforeTheConnection() {
        let client = FakeCoreClient()
        let store = SourcesStore(client: client, tracker: CommandTracker(), toasts: ToastCenter())
        let plugin = Fixture.plugin("org.jellyfesh.subsonic")
        let preset = Fixture.preset()
        store.apply(SourcesState(version: 1, plugins: [plugin], presets: [preset], allowCommunity: false))
        store.editSetting(plugin.id, key: "server_url", value: .string("http://old.example"))
        store.connect(preset, address: "http://192.168.1.20:4533", method: .passwordExchange)
        // The address given to connect arrives last, so it is the one the plugin keeps.
        XCTAssertEqual(client.sent, [
            .setPluginSettings(id: plugin.id, json: #"{"server_url":"http://old.example"}"#),
            .connectSource(preset: "navidrome", address: "http://192.168.1.20:4533", method: .passwordExchange),
        ])
    }

    func testASignInSheetOpensInTheWindowItStartedIn() {
        let client = FakeCoreClient()
        let store = SourcesStore(client: client, tracker: CommandTracker(), toasts: ToastCenter())
        store.connect(Fixture.preset(), address: "http://192.168.1.20:4533", method: .passwordExchange, in: .main)
        XCTAssertNotNil(store.signInFlow(in: .main), "the Sources page shows it")
        XCTAssertNil(store.signInFlow(in: .settings), "Settings does not show it as well")
        store.beginSignIn(Fixture.plugin(), method: .passwordExchange)
        XCTAssertNotNil(store.signInFlow(in: .settings), "Settings › Sources starts its own flows")
        XCTAssertNil(store.signInFlow(in: .main))
    }

    func testHomeKnowsWhetherASourceIsSignedIn() {
        let store = SourcesStore(client: FakeCoreClient(), tracker: CommandTracker(), toasts: ToastCenter())
        var signedIn = Fixture.plugin()
        signedIn.account = "ada"
        store.apply(SourcesState(version: 1, plugins: [Fixture.plugin("org.example.other")], allowCommunity: false))
        XCTAssertFalse(store.hasSignedInSource)
        store.apply(SourcesState(version: 2, plugins: [signedIn], allowCommunity: false))
        XCTAssertTrue(store.hasSignedInSource)
        signedIn.enabled = false
        store.apply(SourcesState(version: 3, plugins: [signedIn], allowCommunity: false))
        XCTAssertFalse(store.hasSignedInSource, "a source that is off brings no music")
    }

    func testSourceNames() {
        let store = SourcesStore(client: FakeCoreClient(), tracker: CommandTracker(), toasts: ToastCenter())
        store.apply(SourcesState(version: 1, plugins: [Fixture.plugin()], allowCommunity: false))
        XCTAssertEqual(store.name(of: "org.example.subsonic"), "Example")
        XCTAssertEqual(store.name(of: "org.example.unknown"), "org.example.unknown")
    }
}

@MainActor
final class QueueStoreTests: XCTestCase {
    func testLibraryTracksAreLookedUpForTheirRows() async {
        let client = FakeCoreClient()
        client.songsByID = [1: Fixture.song(1, title: "First"), 2: Fixture.song(2, title: "Second", artwork: "h2")]
        let store = QueueStore(client: client)
        let entries = [Fixture.entry(20, track: 2), QueueEntry(uid: 21, track: .file(path: "/Music/Loose/Song.flac"))]
        store.apply(Fixture.queue(position: 0, length: 3, current: Fixture.entry(10, track: 1), upNext: entries))
        XCTAssertEqual(store.upNextRows[1].title, "Song", "a file shows its name")
        await eventually { store.current?.title == "First" && store.upNextRows.first?.title == "Second" }
        XCTAssertEqual(store.upNextRows.first?.artwork, .hash("h2"))
        XCTAssertEqual(client.songIDRequests.count, 1, "one trip for the whole batch")
    }

    func testUpNextPagesPastTheWindow() async {
        let client = FakeCoreClient()
        let window = (1...50).map { Fixture.entry(UInt64(100 + $0), track: Int64($0)) }
        client.queueWindowResult = [Fixture.entry(200, track: 60), Fixture.entry(150, track: 50)]
        let store = QueueStore(client: client)
        store.apply(Fixture.queue(position: 0, length: 120, current: Fixture.entry(100, track: 99), upNext: window))
        XCTAssertTrue(store.hasMore)
        await store.loadMore()
        XCTAssertEqual(client.queueWindowRequests.first?.offset, 51)
        XCTAssertEqual(store.upNext.count, 51, "an entry already shown is not added twice")
        XCTAssertEqual(store.upNext.last?.uid, 200)
    }
}

@MainActor
final class SignInStepTests: XCTestCase {
    func testAQuickConnectCodeIsAStepOfItsOwn() {
        let client = FakeCoreClient()
        let store = SourcesStore(client: client, tracker: CommandTracker(), toasts: ToastCenter())
        let plugin = Fixture.plugin(methods: [.qr])
        store.beginSignIn(plugin, method: .qr)
        let step = SignInStep.showCode(code: "481 516", hint: "Enter it under Quick Connect in Jellyfin.",
                                       expiresAtMs: nil)
        store.apply(signIn: SignInUpdate(plugin: plugin.id, flow: 5, step: step))
        XCTAssertEqual(store.signIn?.step, step)
        XCTAssertFalse(store.signIn?.isOver ?? true, "the core still waits for the approval")
        store.endSignIn()
        XCTAssertEqual(client.sent.last, .cancelLogin(plugin: plugin.id, flow: 5))
    }

    func testAQRCodeKeepsThePluginsSentence() {
        let store = SourcesStore(client: FakeCoreClient(), tracker: CommandTracker(), toasts: ToastCenter())
        let plugin = Fixture.plugin(methods: [.qr])
        store.beginSignIn(plugin, method: .qr)
        let step = SignInStep.showQR(data: "https://example.com/qr", hint: "Scan with Example.", expiresAtMs: 99)
        store.apply(signIn: SignInUpdate(plugin: plugin.id, flow: 2, step: step))
        guard case .showQR(_, let hint, let expires)? = store.signIn?.step else { return XCTFail("expected a QR step") }
        XCTAssertEqual(hint, "Scan with Example.")
        XCTAssertEqual(expires, 99)
    }
}

final class SignInFormTests: XCTestCase {
    private let user = SignInField(name: "username", label: "User Name", secret: false, placeholder: nil)
    private let password = SignInField(name: "password", label: "Password", secret: true, placeholder: nil)
    private let note = SignInField(name: "device", label: "Device Name", kind: .text, secret: false, required: false,
                                   placeholder: "Mac", help: "Shown in the server's dashboard.")

    func testSignInWaitsForEveryRequiredField() {
        let fields = [user, password, note]
        XCTAssertFalse(SignInForm.canSubmit(fields, values: [:]))
        XCTAssertFalse(SignInForm.canSubmit(fields, values: ["username": "ada", "password": "  "]),
                       "spaces are not a password")
        XCTAssertTrue(SignInForm.canSubmit(fields, values: ["username": "ada", "password": "secret"]),
                      "an optional field may stay empty")
        XCTAssertTrue(SignInForm.canSubmit([], values: [:]))
    }

    func testANumberFieldKeepsDigits() {
        let code = SignInField(name: "code", label: "Code", kind: .number, secret: false, placeholder: nil)
        XCTAssertEqual(SignInForm.accepted("12 a-3", for: code), "123")
        XCTAssertEqual(SignInForm.accepted("１２３", for: code), "123", "full-width digits from an input method")
        XCTAssertEqual(SignInForm.accepted("ada 12", for: user), "ada 12", "other fields keep what is typed")
    }

    func testTheLineUnderALabelMarksOptionalFieldsAndCarriesTheHelp() {
        XCTAssertNil(SignInForm.note(for: user))
        XCTAssertEqual(SignInForm.note(for: note),
                       "\(String(localized: "Optional")) · Shown in the server's dashboard.")
        var helped = user
        helped.help = "The name you sign in to the server with."
        XCTAssertEqual(SignInForm.note(for: helped), "The name you sign in to the server with.")
        var bare = note
        bare.help = nil
        XCTAssertEqual(SignInForm.note(for: bare), String(localized: "Optional"))
    }

    func testTheKindPicksTheFieldAndWhatTheSystemFillsIn() {
        let cookie = SignInField(name: "cookie", label: "Cookie", kind: .cookieHeader, secret: true, placeholder: nil)
        XCTAssertTrue(SignInForm.isMultiline(cookie))
        XCTAssertFalse(SignInForm.isMultiline(password))
        XCTAssertNil(SignInForm.contentType(of: cookie))
        XCTAssertEqual(SignInForm.contentType(of: password), .password)
        XCTAssertEqual(SignInForm.contentType(of: user), .username)
        let key = SignInField(name: "api_key", label: "API Key", secret: true, placeholder: nil)
        XCTAssertNil(SignInForm.contentType(of: key), "an API key is not a password to save")
        let mail = SignInField(name: "mail", label: "Email", kind: .email, secret: false, placeholder: nil)
        XCTAssertEqual(SignInForm.contentType(of: mail), .emailAddress)
    }

    func testACodeIsSpelledOutForVoiceOver() {
        XCTAssertEqual(SignInForm.spelledOut("AB 12"), "A B 1 2")
    }
}

@MainActor
final class SourcePageTests: XCTestCase {
    private func song(_ id: String, playable: Bool = true) -> SourceResult {
        var item = Fixture.result(id, source: "p", track: .source(SourceSong(plugin: "p", id: id, title: id)))
        item.playable = playable
        return item
    }

    func testAnAlbumPageNumbersItsSongsAndKnowsWhenEveryPageIsIn() {
        let client = FakeCoreClient()
        let sources = SourcesStore(client: client, tracker: CommandTracker(), toasts: ToastCenter())
        let model = BrowseModel(plugin: "p", node: "album-1")
        model.start(sources)
        XCTAssertEqual(client.sent.last, .browse(plugin: "p", node: "album-1", page: 0))
        XCTAssertFalse(model.isComplete)

        let folder = Fixture.result("f", source: "p", kind: .folder)
        sources.apply(browse: BrowseBatch(request: client.lastCommandID, plugin: "p", node: "album-1",
                                          items: [song("a"), folder, song("b")], nextPage: 1, error: nil))
        XCTAssertTrue(model.loaded)
        XCTAssertFalse(model.isComplete, "another page is to come")
        model.loadMore(sources)
        XCTAssertEqual(client.sent.last, .browse(plugin: "p", node: "album-1", page: 1))
        sources.apply(browse: BrowseBatch(request: client.lastCommandID, plugin: "p", node: "album-1",
                                          items: [song("c", playable: false)], nextPage: nil, error: nil))
        XCTAssertTrue(model.isComplete)

        let ids = model.songs.map(\.id)
        XCTAssertEqual(ids.count, 3, "folders are not songs")
        XCTAssertEqual(model.songNumbers(), [ids[0]: 1, ids[1]: 2, ids[2]: 3])
        XCTAssertEqual(model.playableTracks.count, 2, "a song that cannot play here joins no queue")
    }

    func testAContainerPlaysWhileOneOfItsSongsCan() {
        let album = Fixture.result("album-1", source: "p", kind: .album)
        XCTAssertTrue(SourcePage.canPlay(item: album, songs: [], complete: false), "before its songs are in")
        XCTAssertTrue(SourcePage.canPlay(item: album, songs: [song("a", playable: false), song("b")], complete: true))
        XCTAssertFalse(SourcePage.canPlay(item: album, songs: [song("a", playable: false)], complete: true),
                       "none of its songs can play here: its main action opens it in its service")
        var locked = album
        locked.playable = false
        XCTAssertFalse(SourcePage.canPlay(item: locked, songs: [song("a")], complete: false))
    }

    func testOnlyASourceThatAsksIsCredited() {
        let sources = SourcesStore(client: FakeCoreClient(), tracker: CommandTracker(), toasts: ToastCenter())
        var asks = Fixture.plugin("org.example.spotify")
        asks.attribution = "Spotify"
        var blank = Fixture.plugin("org.example.blank")
        blank.attribution = " "
        sources.apply(SourcesState(version: 1, plugins: [asks, blank, Fixture.plugin()], allowCommunity: false))
        XCTAssertEqual(sources.attribution(of: "org.example.spotify"), String(localized: "Content from \("Example")"))
        XCTAssertNil(sources.attribution(of: "org.example.blank"), "a blank attribution asks nothing")
        XCTAssertNil(sources.attribution(of: "org.example.subsonic"))
    }

    func testOtherSourcesAreNamed() {
        let sources = SourcesStore(client: FakeCoreClient(), tracker: CommandTracker(), toasts: ToastCenter())
        sources.apply(SourcesState(version: 1, plugins: [Fixture.plugin()], allowCommunity: false))
        XCTAssertNil(sources.alsoIn([]))
        XCTAssertEqual(sources.alsoIn(["org.example.subsonic"]), String(localized: "Also in \("Example")"))
    }

    func testAnAlbumRowIsNumbered() {
        let row = SourcePage.trackItem(song("a"), artwork: nil, number: 4)
        XCTAssertEqual(row.number, 4)
        XCTAssertEqual(SourcePage.kindName(.album), String(localized: "Album"))
    }
}
