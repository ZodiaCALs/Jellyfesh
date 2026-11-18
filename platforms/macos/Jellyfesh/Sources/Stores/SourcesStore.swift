// Sources (docs/core-api.md): installed plugins with their badges, enabling, sign-in flows, the
// services of Add a Source, settings, installing and browsing. The core drives every flow; this
// keeps what it reported and sends the person's choices. Source commands answer "not available in
// this build yet" until the core integrates them during v0.0.3; that answer shows as a calm
// notice, not an error.

import Foundation
import Observation

/// The window whose sign-in sheet follows a flow: Settings (Sources), or the main window (its
/// Sources page). The flow's sheet opens where it was started, and only there.
enum SignInWindow: Hashable {
    case settings, main
}

/// A sign-in flow in progress, as the sign-in sheet shows it.
struct SignInFlow: Identifiable, Equatable {
    let id: UUID
    var plugin: String
    var pluginName: String
    var method: SignInMethod
    var window: SignInWindow = .settings
    /// The core's flow id, from its first step.
    var flow: UInt64?
    /// The step to show; nil while the core prepares the first one.
    var step: SignInStep?
    /// The core cannot sign in to sources in this build yet.
    var unavailable = false
    /// The command failed for another reason.
    var error: String?

    /// True once the flow has nothing left to do (signed in, failed or not possible).
    var isOver: Bool {
        if unavailable || error != nil { return true }
        switch step {
        case .done?, .failed?: return true
        default: return false
        }
    }
}

@MainActor
@Observable
final class SourcesStore {
    let client: CoreClient
    private let tracker: CommandTracker
    private let toasts: ToastCenter

    private(set) var sources: SourcesState = .empty
    /// Set once a source command answered that sources are not in this build yet.
    private(set) var unavailable = false
    /// The sign-in flow the sheet shows.
    var signIn: SignInFlow?
    /// Players the app provides (Apple Music), by source id: their names. They play like remote
    /// sources (ADR-0018 item 5).
    private(set) var externalSources: [String: String] = [:]

    @ObservationIgnored private var browseHandlers: [CommandID: (BrowseBatch) -> Void] = [:]
    @ObservationIgnored private var browseOrder: [CommandID] = []
    /// Values typed into plugin settings and not sent yet, by plugin and key. Text fields send them
    /// on Return, when they lose focus, and before a sign-in starts, so an address typed and then
    /// followed by a click on Sign In is the one used.
    @ObservationIgnored private var editedSettings: [String: [String: JSONValue]] = [:]
    /// Settings sent and not yet reported back, by plugin, so a quick second change builds on the
    /// first instead of on the older settings.
    @ObservationIgnored private var sentSettings: [String: String] = [:]

    init(client: CoreClient, tracker: CommandTracker, toasts: ToastCenter) {
        self.client = client
        self.tracker = tracker
        self.toasts = toasts
    }

    func apply(_ state: SourcesState) {
        sources = state
        sentSettings = [:]
    }

    var plugins: [SourcePlugin] { sources.plugins }
    /// The services Add a Source offers.
    var presets: [SourcePreset] { sources.presets }
    /// Some source is on and signed in (Home welcomes the person only while none is).
    var hasSignedInSource: Bool { sources.plugins.contains { $0.enabled && $0.account != nil } }

    /// The sign-in flow whose sheet `window` shows, if any.
    func signInFlow(in window: SignInWindow) -> SignInFlow? {
        signIn?.window == window ? signIn : nil
    }

    func plugin(_ id: String) -> SourcePlugin? {
        sources.plugins.first { $0.id == id }
    }

    /// The name to show for a source id (`library` is the local library).
    func name(of source: String) -> String {
        if source == SearchStore.library { return String(localized: "Library") }
        if let name = externalSources[source] { return name }
        return plugin(source)?.name ?? source
    }

    /// Whether a source plays on the person's own devices (a remote plugin, or a player the app
    /// provides): its songs start a remote session and never join the queue, so Play Next, Play
    /// Last, Add to Playlist and the favourites are not offered for them.
    func isRemote(_ source: String) -> Bool {
        externalSources[source] != nil || plugin(source)?.remotePlayback == true
    }

    /// Whether a track is a remote source's.
    func isRemote(_ track: TrackReference?) -> Bool {
        if case .source(let song)? = track { return isRemote(song.plugin) }
        return false
    }

    /// "Content from {Service}" for a source whose terms ask its views to credit it (a blank
    /// attribution asks nothing, as in the other apps).
    func attribution(of source: String) -> String? {
        guard let asked = plugin(source)?.attribution,
              !asked.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
        else { return nil }
        return String(localized: "Content from \(name(of: source))")
    }

    /// "Also in Navidrome, Spotify": the other sources that have an item the core merged in unified
    /// search (`SourceItem.also_in`); nil when there are none.
    func alsoIn(_ others: [String]) -> String? {
        guard !others.isEmpty else { return nil }
        let names = others.map { name(of: $0) }.joined(separator: ", ")
        return String(localized: "Also in \(names)")
    }

    /// A player the app provides was registered with the core (or removed, with nil).
    func setExternalSource(_ id: String, name: String?) {
        externalSources[id] = name
    }

    // MARK: Commands

    func install(path: String) {
        send(.installPlugin(path: path))
    }

    func uninstall(_ id: String) {
        send(.uninstallPlugin(id: id))
    }

    /// Turns a plugin on or off. A community plugin turned on for the first time needs the
    /// disclosure first (ADR-0012): the view asks, then calls `acknowledgeAndEnable`.
    func setEnabled(_ id: String, _ enabled: Bool) {
        send(.setPluginEnabled(id: id, enabled: enabled))
    }

    func acknowledgeAndEnable(_ id: String) {
        send(.acknowledgeUnofficial(id: id))
        send(.setPluginEnabled(id: id, enabled: true))
    }

    func saveSettings(_ id: String, json: String) {
        send(.setPluginSettings(id: id, json: json))
    }

    /// Notes a value typed into one of a plugin's settings; `commitSettings` sends it.
    func editSetting(_ id: String, key: String, value: JSONValue) {
        editedSettings[id, default: [:]][key] = value
    }

    /// Sets one of a plugin's settings at once (switches, choices, numbers), with anything typed.
    func setSetting(_ id: String, key: String, value: JSONValue) {
        editSetting(id, key: key, value: value)
        commitSettings(id)
    }

    /// Sends what was typed into a plugin's settings, when it changes anything.
    func commitSettings(_ id: String) {
        guard let edits = editedSettings.removeValue(forKey: id), !edits.isEmpty,
              let base = sentSettings[id] ?? plugin(id)?.settingsJSON
        else { return }
        var json = base
        for (key, value) in edits.sorted(by: { $0.key < $1.key }) {
            json = SchemaForm.withValue(settingsJSON: json, key: key, value: value)
        }
        guard (try? JSONValue.parse(json)) != (try? JSONValue.parse(base)) else { return }
        sentSettings[id] = json
        saveSettings(id, json: json)
    }

    func logout(_ id: String) {
        send(.logout(plugin: id))
    }

    // MARK: Sign-in

    func beginSignIn(_ plugin: SourcePlugin, method: SignInMethod, in window: SignInWindow = .settings) {
        // The core handles commands in order: an address still in its field arrives first.
        commitSettings(plugin.id)
        let flowID = startFlow(plugin: plugin.id, pluginName: plugin.name, method: method, window: window)
        watchSignIn(client.send(.beginLogin(plugin: plugin.id, method: method)), flow: flowID)
    }

    /// Whether `connect` can go ahead: a service on the person's own server needs its address.
    func canConnect(_ preset: SourcePreset, address: String) -> Bool {
        !preset.needsAddress || !address.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
    }

    /// Connects to a service from Add a Source. The core sets the address, turns the plugin on and
    /// signs in as for `beginSignIn`, so the sign-in sheet follows the same steps, under the
    /// service's name, in the window it was chosen in.
    func connect(_ preset: SourcePreset, address: String, method: SignInMethod,
                 in window: SignInWindow = .settings) {
        guard canConnect(preset, address: address) else { return }
        // Anything still typed into the plugin's settings goes first, so it cannot replace this
        // address afterwards.
        commitSettings(preset.plugin)
        let flowID = startFlow(plugin: preset.plugin, pluginName: preset.name, method: method, window: window)
        let trimmed = address.trimmingCharacters(in: .whitespacesAndNewlines)
        let command = CoreCommand.connectSource(preset: preset.id, address: preset.needsAddress ? trimmed : nil,
                                                method: method)
        watchSignIn(client.send(command), flow: flowID)
    }

    /// Shows a new flow in the sign-in sheet and returns its id. A flow the core is still running
    /// is cancelled first: the sheet follows one flow at a time.
    private func startFlow(plugin: String, pluginName: String, method: SignInMethod,
                           window: SignInWindow) -> UUID {
        if let current = signIn, !current.isOver, let flow = current.flow {
            client.send(.cancelLogin(plugin: current.plugin, flow: flow))
        }
        let flowID = UUID()
        signIn = SignInFlow(id: flowID, plugin: plugin, pluginName: pluginName, method: method, window: window)
        return flowID
    }

    /// A failed sign-in command ends its flow with the reason (or the "not available" notice).
    private func watchSignIn(_ command: CommandID, flow flowID: UUID) {
        tracker.watch(command) { [weak self] message in
            guard let self, self.signIn?.id == flowID else { return }
            if isNotAvailableMessage(message) {
                self.unavailable = true
                self.signIn?.unavailable = true
            } else {
                self.signIn?.error = message
            }
        }
    }

    /// A step of a sign-in flow.
    func apply(signIn update: SignInUpdate) {
        guard var flow = signIn, flow.plugin == update.plugin else { return }
        if let known = flow.flow, known != update.flow { return }
        flow.flow = update.flow
        flow.step = update.step
        signIn = flow
    }

    func submitSignIn(fields: [String: String]) {
        guard let flow = signIn, let id = flow.flow else { return }
        signIn?.step = .waiting(String(localized: "Signing in…"))
        watchSignIn(client.send(.submitLogin(plugin: flow.plugin, flow: id, fields: fields)), flow: flow.id)
    }

    /// Closes the sheet; a flow that has not finished is cancelled.
    func endSignIn() {
        if let flow = signIn, !flow.isOver, let id = flow.flow {
            client.send(.cancelLogin(plugin: flow.plugin, flow: id))
        }
        signIn = nil
    }

    // MARK: Browse

    /// Asks a source for one page of a container; `onPage` gets the answer, `onFailure` a failed
    /// command (with `unavailable` set when sources are not in this build yet).
    @discardableResult
    func browse(plugin: String, node: String?, page: UInt32, onPage: @escaping (BrowseBatch) -> Void,
                onFailure: @escaping (String) -> Void) -> CommandID {
        let id = client.send(.browse(plugin: plugin, node: node, page: page))
        guard id != 0 else {
            onFailure(String(localized: "The player has stopped."))
            return 0
        }
        browseHandlers[id] = onPage
        browseOrder.append(id)
        if browseOrder.count > 64 {
            browseHandlers[browseOrder.removeFirst()] = nil
        }
        tracker.watch(id) { [weak self] message in
            if isNotAvailableMessage(message) { self?.unavailable = true }
            self?.browseHandlers[id] = nil
            onFailure(message)
        }
        return id
    }

    func apply(browse batch: BrowseBatch) {
        browseHandlers[batch.request]?(batch)
    }

    // MARK: -

    private func send(_ command: CoreCommand) {
        let id = client.send(command)
        tracker.watch(id) { [weak self] message in
            guard let self else { return }
            if isNotAvailableMessage(message) {
                self.unavailable = true
            } else {
                self.toasts.show(message)
            }
        }
    }
}

/// One container of a source's catalogue, page by page.
@MainActor
@Observable
final class BrowseModel {
    let plugin: String
    let node: String?
    private(set) var items: [SourceResult] = []
    private(set) var nextPage: UInt32?
    private(set) var isLoading = false
    private(set) var error: String?
    private(set) var unavailable = false
    /// The first page has arrived.
    private(set) var loaded = false

    init(plugin: String, node: String?) {
        self.plugin = plugin
        self.node = node
    }

    /// Every page is here: the songs can be counted and added to a playlist.
    var isComplete: Bool { loaded && nextPage == nil && !isLoading }

    /// The songs of the pages loaded so far, in order.
    var songs: [SourceResult] { items.filter { $0.kind == .track } }

    /// The songs that can join the queue and playlists: playable, with a track.
    var playableTracks: [TrackReference] {
        items.filter { $0.kind == .track && $0.playable }.compactMap(\.track)
    }

    func start(_ sources: SourcesStore) {
        guard items.isEmpty, !isLoading else { return }
        request(page: 0, sources)
    }

    func loadMore(_ sources: SourcesStore) {
        guard let page = nextPage, !isLoading else { return }
        request(page: page, sources)
    }

    private func request(page: UInt32, _ sources: SourcesStore) {
        isLoading = true
        error = nil
        sources.browse(plugin: plugin, node: node, page: page, onPage: { [weak self] batch in
            self?.received(batch)
        }, onFailure: { [weak self] message in
            guard let self else { return }
            self.isLoading = false
            if isNotAvailableMessage(message) {
                self.unavailable = true
            } else {
                self.error = message
            }
        })
    }

    private func received(_ batch: BrowseBatch) {
        isLoading = false
        if let message = batch.error {
            error = message
            return
        }
        let known = Set(items.map(\.id))
        items.append(contentsOf: batch.items.filter { !known.contains($0.id) })
        nextPage = batch.nextPage
        loaded = true
    }

    /// The song numbers of an album page: each song's place among the page's songs, from 1.
    func songNumbers() -> [String: UInt32] {
        var numbers: [String: UInt32] = [:]
        for (index, song) in songs.enumerated() where numbers[song.id] == nil {
            numbers[song.id] = UInt32(index + 1)
        }
        return numbers
    }

    /// The tracks of this page in order, and where `item` is among them (to play from it).
    func tracks(startingAt item: SourceResult) -> (tracks: [TrackReference], start: Int)? {
        let playable = items.filter { $0.kind == .track && $0.playable && $0.track != nil }
        guard let start = playable.firstIndex(where: { $0.id == item.id }) else { return nil }
        return (playable.compactMap(\.track), start)
    }
}
