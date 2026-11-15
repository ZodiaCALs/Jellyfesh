// What the system says about drawing: Low Power Mode (glass turns solid, ADR-0007) and whether
// the main window can be seen (Still mode: no time-driven drawing while it cannot), plus the
// shared keys the menus cannot carry (Space, Shift-arrows and Esc).

import AppKit
import Observation
import SwiftUI

@MainActor
@Observable
final class SystemState {
    private(set) var lowPower = ProcessInfo.processInfo.isLowPowerModeEnabled
    /// The main window is on screen and not hidden behind others or minimised.
    var windowVisible = true
    /// The main window (the library and Now Playing), as opposed to Settings, sheets and panels.
    @ObservationIgnored weak var mainWindow: NSWindow?

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

/// Reports whether the window holding it can be seen: on screen, not minimised, not covered; and
/// which window that is.
struct WindowVisibilityReader: NSViewRepresentable {
    var onChange: (Bool) -> Void
    var onWindow: ((NSWindow) -> Void)? = nil

    func makeNSView(context: Context) -> VisibilityView {
        let view = VisibilityView()
        view.onChange = onChange
        view.onWindow = onWindow
        return view
    }

    func updateNSView(_ view: VisibilityView, context: Context) {
        view.onChange = onChange
        view.onWindow = onWindow
    }

    final class VisibilityView: NSView {
        var onChange: ((Bool) -> Void)?
        var onWindow: ((NSWindow) -> Void)?
        private var lastReported: Bool?

        override func viewDidMoveToWindow() {
            super.viewDidMoveToWindow()
            NotificationCenter.default.removeObserver(self)
            guard let window else { return }
            onWindow?(window)
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

/// Key codes of the shared keys (docs/ui.md, "Keyboard") the menus cannot carry.
private enum SharedKeyCode {
    static let space: UInt16 = 49
    static let escape: UInt16 = 53
    static let leftArrow: UInt16 = 123
    static let rightArrow: UInt16 = 124
}

/// The shared keys (docs/ui.md, "Keyboard") that the menus cannot carry, because a menu's key
/// equivalent would take them from text fields:
/// * Space plays and pauses, as in other players, except when Full Keyboard Access is on (Space
///   then presses the focused control);
/// * Shift-Right and Shift-Left skip 5 seconds forward and back (`skip` answers whether it did:
///   not with nothing playing);
/// * Esc goes back a page in the main window (`back` answers whether it did); Now Playing closes
///   on its own Esc.
/// None of them while text is edited, where they type, select or cancel, nor in sheets and panels.
@MainActor
final class SharedKeyMonitor {
    private var monitor: Any?

    func start(playPause: @escaping @MainActor () -> Void,
               skip: @escaping @MainActor (_ forward: Bool) -> Bool,
               back: @escaping @MainActor (NSWindow) -> Bool) {
        guard monitor == nil else { return }
        monitor = NSEvent.addLocalMonitorForEvents(matching: .keyDown) { event in
            // Arrow keys carry the numeric pad and function flags; Caps Lock changes nothing here.
            let modifiers = event.modifierFlags.intersection(.deviceIndependentFlagsMask)
                .subtracting([.capsLock, .numericPad, .function])
            let key = event.keyCode
            let isSpace = key == SharedKeyCode.space && modifiers.isEmpty
            let isEscape = key == SharedKeyCode.escape && modifiers.isEmpty
            let isShiftArrow = (key == SharedKeyCode.leftArrow || key == SharedKeyCode.rightArrow)
                && modifiers == .shift
            guard isSpace || isEscape || isShiftArrow else { return event }
            let handled = MainActor.assumeIsolated { () -> Bool in
                let app = NSApplication.shared
                guard let window = app.keyWindow else { return false }
                if window.firstResponder is NSText { return false }
                // Sheets and panels (sign-in, file pickers, alerts) keep their keys.
                if window.isSheet || window is NSPanel || window.attachedSheet != nil { return false }
                if isShiftArrow {
                    // Held down, it keeps skipping, as a menu shortcut would.
                    return skip(key == SharedKeyCode.rightArrow)
                }
                if event.isARepeat { return false }
                if isEscape {
                    return back(window)
                }
                if app.isFullKeyboardAccessEnabled { return false }
                playPause()
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
