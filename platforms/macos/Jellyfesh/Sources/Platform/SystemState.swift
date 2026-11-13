// What the system says about drawing: Low Power Mode (glass turns solid, ADR-0007) and whether
// the main window can be seen (Still mode: no time-driven drawing while it cannot), plus the
// Space bar for play and pause.

import AppKit
import Observation
import SwiftUI

@MainActor
@Observable
final class SystemState {
    private(set) var lowPower = ProcessInfo.processInfo.isLowPowerModeEnabled
    /// The main window is on screen and not hidden behind others or minimised.
    var windowVisible = true

    @ObservationIgnored private var observer: PowerObserver?

    func start() {
        guard observer == nil else { return }
        observer = PowerObserver { [weak self] in
            self?.lowPower = ProcessInfo.processInfo.isLowPowerModeEnabled
        }
    }
}

/// Calls back when Low Power Mode turns on or off.
private final class PowerObserver: NSObject {
    private let changed: @MainActor () -> Void

    init(changed: @escaping @MainActor () -> Void) {
        self.changed = changed
        super.init()
        NotificationCenter.default.addObserver(
            self, selector: #selector(powerStateChanged(_:)),
            name: Notification.Name.NSProcessInfoPowerStateDidChange, object: nil
        )
    }

    @objc private func powerStateChanged(_ notification: Notification) {
        // Posted on an arbitrary thread.
        let changed = self.changed
        Task { @MainActor in changed() }
    }
}

/// Reports whether the window holding it can be seen: on screen, not minimised, not covered.
struct WindowVisibilityReader: NSViewRepresentable {
    var onChange: (Bool) -> Void

    func makeNSView(context: Context) -> VisibilityView {
        let view = VisibilityView()
        view.onChange = onChange
        return view
    }

    func updateNSView(_ view: VisibilityView, context: Context) {
        view.onChange = onChange
    }

    final class VisibilityView: NSView {
        var onChange: ((Bool) -> Void)?
        private var lastReported: Bool?

        override func viewDidMoveToWindow() {
            super.viewDidMoveToWindow()
            NotificationCenter.default.removeObserver(self)
            guard let window else { return }
            let names: [Notification.Name] = [
                NSWindow.didChangeOcclusionStateNotification,
                NSWindow.didMiniaturizeNotification,
                NSWindow.didDeminiaturizeNotification,
            ]
            for name in names {
                NotificationCenter.default.addObserver(self, selector: #selector(windowChanged(_:)), name: name,
                                                       object: window)
            }
            report()
        }

        @objc private func windowChanged(_ notification: Notification) {
            report()
        }

        private func report() {
            guard let window else { return }
            let visible = window.occlusionState.contains(.visible) && !window.isMiniaturized
            guard visible != lastReported else { return }
            lastReported = visible
            let onChange = self.onChange
            // Not during a view update.
            DispatchQueue.main.async { onChange?(visible) }
        }
    }
}

/// The Space bar plays and pauses, as in other players, except while text is edited and when
/// Full Keyboard Access is on (Space then presses the focused control).
@MainActor
final class SpaceKeyMonitor {
    private var monitor: Any?

    func start(_ action: @escaping @MainActor () -> Void) {
        guard monitor == nil else { return }
        monitor = NSEvent.addLocalMonitorForEvents(matching: .keyDown) { event in
            let modifiers = event.modifierFlags.intersection(.deviceIndependentFlagsMask)
            guard event.keyCode == 49, modifiers.subtracting(.capsLock).isEmpty, !event.isARepeat else {
                return event
            }
            let handled = MainActor.assumeIsolated { () -> Bool in
                let app = NSApplication.shared
                if app.isFullKeyboardAccessEnabled { return false }
                guard let window = app.keyWindow else { return false }
                if window.firstResponder is NSText { return false }
                // Sheets and panels (sign-in, file pickers) keep their keys.
                if window.isSheet || window is NSPanel || window.attachedSheet != nil { return false }
                action()
                return true
            }
            return handled ? nil : event
        }
    }

    func stop() {
        if let monitor { NSEvent.removeMonitor(monitor) }
        monitor = nil
    }
}
