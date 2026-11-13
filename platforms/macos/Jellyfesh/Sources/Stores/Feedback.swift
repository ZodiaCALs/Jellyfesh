// Command failures and messages for the person.

import AppKit
import Observation

/// Routes `.commandFailed` to whoever sent the command. Commands are fire-and-forget (the core
/// sends nothing on success), so only the latest few are remembered.
@MainActor
final class CommandTracker {
    private var handlers: [CommandID: (String) -> Void] = [:]
    private var order: [CommandID] = []
    private let capacity = 128

    func watch(_ id: CommandID, onFailure: @escaping (String) -> Void) {
        guard id != 0 else {
            onFailure(String(localized: "The player has stopped."))
            return
        }
        handlers[id] = onFailure
        order.append(id)
        if order.count > capacity {
            handlers[order.removeFirst()] = nil
        }
    }

    /// Hands a failure to its sender; false when nobody watched the command.
    func fail(_ id: CommandID, message: String) -> Bool {
        guard let handler = handlers.removeValue(forKey: id) else { return false }
        order.removeAll { $0 == id }
        handler(message)
        return true
    }
}

/// One short message at a time, shown at the top of the window and read out by VoiceOver.
@MainActor
@Observable
final class ToastCenter {
    private(set) var message: String?
    @ObservationIgnored private var hideTask: Task<Void, Never>?

    func show(_ text: String) {
        message = text
        NSAccessibility.post(
            element: NSApplication.shared,
            notification: .announcementRequested,
            userInfo: [.announcement: text, .priority: NSAccessibilityPriorityLevel.high.rawValue]
        )
        hideTask?.cancel()
        hideTask = Task { [weak self] in
            try? await Task.sleep(for: .seconds(5))
            guard !Task.isCancelled else { return }
            self?.message = nil
        }
    }

    func dismiss() {
        hideTask?.cancel()
        message = nil
    }
}
