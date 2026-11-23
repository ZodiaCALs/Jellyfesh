// The mini player's window (ADR-0026 §4), built as Desktop Lyrics' is: a small borderless panel
// that does not make Jellyfesh the active app, on every Space and beside full-screen apps, at the
// floating level while Keep on Top is on (a normal window otherwise). It is dragged by its
// background and keeps its place between launches; it first opens at the top right of the screen.
// Its glass is the system's behind-window material with the Veil tint, rounded to the mini
// player's corners. What it shows is MiniPlayerView; when it shows is MiniPlayerStore's.

import AppKit
import SwiftUI

@MainActor
final class MiniPlayerWindow: MiniPlayerHost {
    /// The name its frame is kept under in the defaults.
    static let frameName = "Mini Player"
    /// The window's size: the cover, three lines and the transport, the progress line under them.
    static let size = CGSize(width: 340, height: 122)

    private let content: () -> AnyView
    private var panel: NSPanel?
    private var keepOnTop = true

    /// `content` makes the window's view the first time it shows.
    init(content: @escaping () -> AnyView) {
        self.content = content
    }

    func show() {
        let panel = self.panel ?? makePanel()
        self.panel = panel
        applyLevel(panel)
        panel.orderFrontRegardless()
    }

    func hide() {
        panel?.orderOut(nil)
    }

    func setKeepOnTop(_ keepOnTop: Bool) {
        self.keepOnTop = keepOnTop
        if let panel {
            applyLevel(panel)
        }
    }

    private func applyLevel(_ panel: NSPanel) {
        panel.isFloatingPanel = keepOnTop
        panel.level = keepOnTop ? .floating : .normal
    }

    private func makePanel() -> NSPanel {
        let screen = NSScreen.main?.visibleFrame ?? CGRect(x: 0, y: 0, width: 1440, height: 900)
        let panel = NSPanel(contentRect: Self.defaultFrame(in: screen), styleMask: [.borderless, .nonactivatingPanel],
                            backing: .buffered, defer: true)
        panel.collectionBehavior = [.canJoinAllSpaces, .fullScreenAuxiliary, .ignoresCycle]
        // Panels hide when their app is not active unless told otherwise.
        panel.hidesOnDeactivate = false
        panel.isMovableByWindowBackground = true
        panel.isReleasedWhenClosed = false
        panel.isOpaque = false
        panel.backgroundColor = .clear
        panel.hasShadow = true
        panel.title = String(localized: "Mini Player")
        panel.contentView = NSHostingView(rootView: content())
        // Back where the person left it.
        _ = panel.setFrameUsingName(Self.frameName)
        _ = panel.setFrameAutosaveName(Self.frameName)
        return panel
    }

    /// At the top right of the visible screen, clear of its edges.
    static func defaultFrame(in visible: CGRect) -> CGRect {
        let width = min(size.width, visible.width)
        return CGRect(x: visible.maxX - width - 24, y: visible.maxY - size.height - 24,
                      width: width, height: size.height)
    }
}

/// The system's behind-window material (it follows the window's appearance and Reduce
/// Transparency), rounded with a mask so the blur itself has the corners.
struct BehindWindowMaterial: NSViewRepresentable {
    var cornerRadius: CGFloat

    func makeNSView(context: Context) -> NSVisualEffectView {
        let view = NSVisualEffectView()
        view.material = .popover
        view.blendingMode = .behindWindow
        view.state = .active
        view.maskImage = Self.mask(radius: cornerRadius)
        return view
    }

    func updateNSView(_ view: NSVisualEffectView, context: Context) {
        view.maskImage = Self.mask(radius: cornerRadius)
    }

    /// A stretchable rounded rectangle.
    static func mask(radius: CGFloat) -> NSImage {
        let side = 2 * radius + 1
        let image = NSImage(size: NSSize(width: side, height: side), flipped: false) { rect in
            NSColor.black.setFill()
            NSBezierPath(roundedRect: rect, xRadius: radius, yRadius: radius).fill()
            return true
        }
        image.capInsets = NSEdgeInsets(top: radius, left: radius, bottom: radius, right: radius)
        image.resizingMode = .stretch
        return image
    }
}
