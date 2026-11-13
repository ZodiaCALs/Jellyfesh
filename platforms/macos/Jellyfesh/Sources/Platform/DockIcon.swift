// The Dock icon (ADR-0017). The bundle icon is Ice. Automatic shows Steel while the system is
// dark; any other choice replaces the Dock icon while Jellyfesh runs. Finder and Launchpad keep
// the bundle icon: the app bundle is never modified, which would break its signature.

import AppKit

@MainActor
final class DockIcon: NSObject {
    private var choice = AppIconChoice.automatic.rawValue
    private var applied: String??

    override init() {
        super.init()
        DistributedNotificationCenter.default().addObserver(
            self,
            selector: #selector(systemAppearanceChanged(_:)),
            name: Notification.Name("AppleInterfaceThemeChangedNotification"),
            object: nil
        )
    }

    /// The Dock follows the system's appearance, not the one the app may force on its windows.
    nonisolated static var systemIsDark: Bool {
        UserDefaults.standard.string(forKey: "AppleInterfaceStyle")?.lowercased() == "dark"
    }

    /// The asset to show for `choice` (`automatic` or an icon id); nil for the bundle icon.
    nonisolated static func assetName(for choice: String, systemDark: Bool) -> String? {
        switch choice {
        case AppIconChoice.automatic.rawValue:
            return systemDark ? "DockIcon-steel" : nil
        case AppIconChoice.ice.rawValue:
            return nil
        default:
            guard AppIconChoice(rawValue: choice) != nil else { return nil }
            return "DockIcon-\(choice)"
        }
    }

    func apply(_ choice: String) {
        self.choice = choice
        refresh()
    }

    @objc private func systemAppearanceChanged(_ notification: Notification) {
        // The defaults change just after the notification.
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.1) { [weak self] in
            MainActor.assumeIsolated {
                self?.refresh()
            }
        }
    }

    private func refresh() {
        let name = Self.assetName(for: choice, systemDark: Self.systemIsDark)
        guard applied != .some(name) else { return }
        applied = .some(name)
        // nil restores the bundle icon.
        NSApplication.shared.applicationIconImage = name.flatMap { NSImage(named: $0) }
    }
}
