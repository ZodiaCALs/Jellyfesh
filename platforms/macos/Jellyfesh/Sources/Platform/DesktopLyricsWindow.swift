// Desktop Lyrics' window (ADR-0022 §6): a borderless panel that floats above other apps' windows
// without making Jellyfesh the active app, shows on every Space and beside full-screen apps, and
// lets clicks pass through while it is locked. It is dragged by its background and keeps its
// place between launches; it first opens centred near the bottom of the screen. What it shows is
// DesktopLyricsView; when it shows is DesktopLyricsStore's.

import AppKit
import SwiftUI

@MainActor
final class DesktopLyricsWindow: DesktopLyricsHost {
    /// The name its frame is kept under in the defaults.
    static let frameName = "Desktop Lyrics"
    /// The window's size: two lines of large lyrics, with room for the controls.
    static let size = CGSize(width: 820, height: 136)

    private let content: () -> AnyView
    private var panel: NSPanel?
    private var clickThrough = false

    /// `content` makes the window's view the first time it shows.
    init(content: @escaping () -> AnyView) {
        self.content = content
    }

    func show() {
        let panel = self.panel ?? makePanel()
        self.panel = panel
        panel.ignoresMouseEvents = clickThrough
        panel.orderFrontRegardless()
    }

    func hide() {
        panel?.orderOut(nil)
    }

    func setClickThrough(_ clickThrough: Bool) {
        self.clickThrough = clickThrough
        panel?.ignoresMouseEvents = clickThrough
    }

    private func makePanel() -> NSPanel {
        let screen = NSScreen.main?.visibleFrame ?? CGRect(x: 0, y: 0, width: 1440, height: 900)
        let panel = NSPanel(contentRect: Self.defaultFrame(in: screen), styleMask: [.borderless, .nonactivatingPanel],
                            backing: .buffered, defer: true)
        panel.isFloatingPanel = true
        panel.level = .floating
        panel.collectionBehavior = [.canJoinAllSpaces, .fullScreenAuxiliary, .ignoresCycle]
        // Panels hide when their app is not active unless told otherwise.
        panel.hidesOnDeactivate = false
        panel.isMovableByWindowBackground = true
        panel.isReleasedWhenClosed = false
        panel.isOpaque = false
        panel.backgroundColor = .clear
        panel.hasShadow = false
        panel.title = String(localized: "Desktop Lyrics")
        panel.contentView = NSHostingView(rootView: content())
        // Back where the person left it.
        _ = panel.setFrameUsingName(Self.frameName)
        _ = panel.setFrameAutosaveName(Self.frameName)
        return panel
    }

    /// Centred, a little above the bottom of the visible screen (above the Dock).
    static func defaultFrame(in visible: CGRect) -> CGRect {
        let width = min(size.width, visible.width)
        return CGRect(x: visible.midX - width / 2, y: visible.minY + 64, width: width, height: size.height)
    }
}

/// Tells when the pointer enters and leaves a view, with a tracking area that stays active while
/// Jellyfesh is not the active app (Desktop Lyrics floats over other apps).
struct PointerHoverReader: NSViewRepresentable {
    var onChange: (Bool) -> Void

    func makeNSView(context: Context) -> HoverView {
        let view = HoverView()
        view.onChange = onChange
        return view
    }

    func updateNSView(_ view: HoverView, context: Context) {
        view.onChange = onChange
    }

    final class HoverView: NSView {
        var onChange: ((Bool) -> Void)?

        override func viewDidMoveToWindow() {
            super.viewDidMoveToWindow()
            guard window != nil, trackingAreas.isEmpty else { return }
            // In the visible rect, so it follows the view's size by itself.
            addTrackingArea(NSTrackingArea(rect: .zero, options: [.mouseEnteredAndExited, .activeAlways, .inVisibleRect],
                                           owner: self, userInfo: nil))
        }

        override func mouseEntered(with event: NSEvent) {
            onChange?(true)
        }

        override func mouseExited(with event: NSEvent) {
            onChange?(false)
        }
    }
}
