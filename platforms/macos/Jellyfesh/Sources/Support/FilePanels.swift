// Open and save panels for folders, playlists and plugin packages, usable from menus as well as
// views. The app is not sandboxed (ADR-0016), so the chosen paths go to the core as they are.

import AppKit
import UniformTypeIdentifiers

@MainActor
enum FilePanels {
    /// The `.jfpkg` plugin package type (Info.plist exports `org.jellyfesh.plugin`).
    static var pluginType: UTType {
        UTType("org.jellyfesh.plugin") ?? UTType(filenameExtension: "jfpkg") ?? .zip
    }

    static var playlistTypes: [UTType] {
        [UTType.m3uPlaylist, UTType(filenameExtension: "m3u8")].compactMap { $0 }
    }

    /// Folders with music to add to the library.
    static func chooseMusicFolders(_ done: @escaping ([URL]) -> Void) {
        let panel = NSOpenPanel()
        panel.title = String(localized: "Add Music Folder")
        panel.prompt = String(localized: "Add")
        panel.canChooseDirectories = true
        panel.canChooseFiles = false
        panel.allowsMultipleSelection = true
        panel.begin { response in
            guard response == .OK else { return }
            done(panel.urls)
        }
    }

    /// An M3U playlist to import.
    static func choosePlaylist(_ done: @escaping (URL) -> Void) {
        let panel = NSOpenPanel()
        panel.title = String(localized: "Import Playlist")
        panel.prompt = String(localized: "Import")
        panel.allowedContentTypes = playlistTypes
        panel.allowsMultipleSelection = false
        panel.begin { response in
            guard response == .OK, let url = panel.url else { return }
            done(url)
        }
    }

    /// A plugin package to install.
    static func choosePlugin(_ done: @escaping (URL) -> Void) {
        let panel = NSOpenPanel()
        panel.title = String(localized: "Install from File")
        panel.prompt = String(localized: "Install")
        panel.allowedContentTypes = [pluginType]
        panel.allowsMultipleSelection = false
        panel.begin { response in
            guard response == .OK, let url = panel.url else { return }
            done(url)
        }
    }

    /// Where to export a playlist as M3U.
    static func savePlaylist(named name: String, _ done: @escaping (URL) -> Void) {
        let panel = NSSavePanel()
        panel.title = String(localized: "Export Playlist")
        panel.nameFieldStringValue = name + ".m3u8"
        panel.allowedContentTypes = playlistTypes
        panel.canCreateDirectories = true
        panel.begin { response in
            guard response == .OK, let url = panel.url else { return }
            done(url)
        }
    }
}
