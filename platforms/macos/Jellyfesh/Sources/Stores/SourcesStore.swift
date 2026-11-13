// Sources (docs/core-api.md): installed plugins with their badges, enabling, sign-in flows,
// settings, installing and browsing. The core drives every flow; this keeps what it reported and
// sends the person's choices. Source commands answer "not available in this build yet" until the
// core integrates them during v0.0.3; that answer shows as a calm notice, not an error.

import Foundation
import Observation

/// A sign-in flow in progress, as the sign-in sheet shows it.
struct SignInFlow: Identifiable, Equatable {
    let id: UUID
    var plugin: String
    var pluginName: String
    var method: SignInMethod
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

    @ObservationIgnored private var browseHandlers: [CommandID: (BrowseBatch) -> Void] = [:]
    @ObservationIgnored private var browseOrder: [CommandID] = []

    init(client: CoreClient, tracker: CommandTracker, toasts: ToastCenter) {
        self.client = client
        self.tracker = tracker
        self.toasts = toasts
    }

    func apply(_ state: SourcesState) {
        sources = state
    }

    var plugins: [SourcePlugin] { sources.plugins }

    func plugin(_ id: String) -> SourcePlugin? {
        sources.plugins.first { $0.id == id }
    }

    /// The name to show for a source id (`library` is the local library).
    func name(of source: String) -> String {
        if source == SearchStore.library { return String(localized: "Library") }
        return plugin(source)?.name ?? source
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

    func logout(_ id: String) {
        send(.logout(plugin: id))
    }

    // MARK: Sign-in

    func beginSignIn(_ plugin: SourcePlugin, method: SignInMethod) {
        if let current = signIn, !current.isOver, let flow = current.flow {
            client.send(.cancelLogin(plugin: current.plugin, flow: flow))
        }
        let flowID = UUID()
        signIn = SignInFlow(id: flowID, plugin: plugin.id, pluginName: plugin.name, method: method)
        watchSignIn(client.send(.beginLogin(plugin: plugin.id, method: method)), flow: flowID)
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

    init(plugin: String, node: String?) {
        self.plugin = plugin
        self.node = node
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
    }

    /// The tracks of this page in order, and where `item` is among them (to play from it).
    func tracks(startingAt item: SourceResult) -> (tracks: [TrackReference], start: Int)? {
        let playable = items.filter { $0.kind == .track && $0.playable && $0.track != nil }
        guard let start = playable.firstIndex(where: { $0.id == item.id }) else { return nil }
        return (playable.compactMap(\.track), start)
    }
}
