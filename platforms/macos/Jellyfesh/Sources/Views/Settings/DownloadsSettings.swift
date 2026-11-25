// Settings › Downloads (ADR-0025, docs/ui.md "Settings"; not in Jellyfesh Web), as the web's cards
// (SettingsCards): where downloads go, with Show in Finder and Change… (a folder chosen in the
// sandboxed build is kept with a security-scoped bookmark); their quality; how the files are named
// (the template, its fields and a live preview of one song's path); what goes in the tags; the
// network (Pause on Metered Connections, as NetworkMonitor reports a hotspot or Low Data Mode, and
// how many download at once); and the space they take, with Remove All Downloads… (asked first).

import AppKit
import SwiftUI

struct DownloadsSettings: View {
    @Environment(DownloadsStore.self) private var downloads
    @Environment(SettingsStore.self) private var settings
    @State private var removingAll = false

    var body: some View {
        SettingsCardsColumn {
            SettingsCard {
                folderRow
                SettingsDivider()
                qualityRow
            }

            SettingsCard(title: "File Names") {
                FileNamesEditor()
            }

            SettingsCard(title: "Tags") {
                SettingsToggleRow("Add Missing Cover and Lyrics",
                                  help: "Original files are kept as they are unless this is on.",
                                  isOn: binding(\.addMissingCoverAndLyrics))
                SettingsDivider()
                SettingsToggleRow("Lyrics", isOn: binding(\.lyricsInTags))
                SettingsDivider()
                SettingsToggleRow("Save .lrc Files", isOn: binding(\.saveLrcFiles))
            }

            SettingsCard(title: "Network") {
                SettingsToggleRow("Pause on Metered Connections",
                                  help: "Downloads wait while you're on mobile data, a hotspot or another metered connection.",
                                  isOn: binding(\.wifiOnly))
                SettingsDivider()
                atOnceRow
            }

            SettingsCard {
                SettingsRow(title: Text("Space Used"), help: Text(verbatim: downloads.spaceUsed)) {
                    Button("Remove All Downloads…") {
                        removingAll = true
                    }
                    .disabled(downloads.state.items.isEmpty)
                }
            }
        }
        .confirmationDialog(Text("Remove All Downloads?"), isPresented: $removingAll, titleVisibility: .visible) {
            Button("Remove All", role: .destructive) {
                downloads.removeAll()
            }
        } message: {
            Text("Every downloaded file is deleted from this device.")
        }
    }

    /// Folder: where downloads go now (the core's answer, else the person's choice), Show in Finder
    /// and Change….
    @ViewBuilder private var folderRow: some View {
        let chosen = settings.current.downloads.folder
        let folder = downloads.state.folder.isEmpty ? (chosen ?? "") : downloads.state.folder
        let shown = DownloadsStore.displayPath(folder, home: DownloadsStore.realHome)
        SettingsRow(title: Text("Folder"), help: folder.isEmpty ? nil : Text(verbatim: shown)) {
            HStack(spacing: Tokens.Space.s2) {
                Button {
                    Self.reveal(folder)
                } label: {
                    Image(systemName: "magnifyingglass")
                }
                .buttonStyle(.borderless)
                .disabled(folder.isEmpty)
                .help(Text("Show in Finder"))
                .accessibilityLabel(Text("Show in Finder"))
                Button("Change…") {
                    FilePanels.chooseDownloadFolder(from: folder) { url in
                        downloads.keepAccess(to: url)
                        settings.update { $0.downloads.folder = url.path }
                    }
                }
            }
        }
        .help(folder)
    }

    /// Quality: Original (the server's file) or one of its conversions.
    private var qualityRow: some View {
        let quality = settings.current.downloads.quality
        return SettingsRow(title: Text("Quality"),
                           help: quality == .original ? Text("The file exactly as your server keeps it.") : nil) {
            Picker("Quality", selection: Binding(
                get: { settings.current.downloads.quality },
                set: { value in settings.update { $0.downloads.quality = value } }
            )) {
                ForEach(DownloadQualityOption.allCases, id: \.self) { option in
                    Text(verbatim: option.title).tag(option)
                }
            }
            .labelsHidden()
            .fixedSize()
        }
    }

    /// Downloads at Once: 1 to 4.
    private var atOnceRow: some View {
        let atOnce = Int(settings.current.downloads.atOnce)
        return SettingsRow("Downloads at Once") {
            Stepper(value: Binding(
                get: { Int(settings.current.downloads.atOnce) },
                set: { value in settings.update { $0.downloads.atOnce = UInt32(clamping: min(max(value, 1), 4)) } }
            ), in: 1...4) {
                Text(verbatim: "\(atOnce)")
                    .monospacedDigit()
            }
            .accessibilityLabel(Text("Downloads at Once"))
            .accessibilityValue(Text(verbatim: "\(atOnce)"))
        }
    }

    private func binding(_ option: WritableKeyPath<DownloadOptions, Bool>) -> Binding<Bool> {
        Binding(
            get: { settings.current.downloads[keyPath: option] },
            set: { value in settings.update { $0.downloads[keyPath: option] = value } }
        )
    }

    /// Show in Finder: the folder, or the nearest folder above it while nothing has downloaded yet
    /// (the core makes it with the first download).
    private static func reveal(_ path: String) {
        var url = URL(fileURLWithPath: path, isDirectory: true)
        while !FileManager.default.fileExists(atPath: url.path), url.pathComponents.count > 1 {
            url.deleteLastPathComponent()
        }
        NSWorkspace.shared.activateFileViewerSelecting([url])
    }
}

/// File Names: the template (saved on Return or on leaving the field), its fields, and a live
/// preview of one song's path under it.
private struct FileNamesEditor: View {
    @Environment(DownloadsStore.self) private var downloads
    @Environment(SettingsStore.self) private var settings
    @Environment(\.palette) private var palette
    @State private var template = ""
    @FocusState private var editing: Bool

    /// The template's fields (ADR-0025 §2).
    static let fields = ["{artist}", "{album_artist}", "{album}", "{title}", "{track}", "{disc}", "{year}", "{source}"]

    var body: some View {
        VStack(alignment: .leading, spacing: Tokens.Space.s2) {
            TextField(text: $template, prompt: Text(verbatim: DownloadOptions.defaults.nameTemplate)) {
                Text("File Names")
            }
            .textFieldStyle(.roundedBorder)
            .labelsHidden()
            .font(.system(size: 13, design: .monospaced))
            .focused($editing)
            .onSubmit(save)
            Text("Fields: \(Self.fields.joined(separator: " "))")
                .lookFont(size: 12)
                .foregroundStyle(palette[Tokens.Ink.ink70])
                .textSelection(.enabled)
                .fixedSize(horizontal: false, vertical: true)
            Text("Preview: \(downloads.preview(template))")
                .lookFont(size: 13)
                .foregroundStyle(palette[Tokens.Ink.ink90])
                .lineLimit(2)
                .truncationMode(.middle)
                .textSelection(.enabled)
        }
        .padding(.vertical, Tokens.Space.s3)
        .onAppear {
            template = settings.current.downloads.nameTemplate
        }
        .onChange(of: editing) { _, now in
            if !now { save() }
        }
        .onChange(of: settings.current.downloads.nameTemplate) { _, value in
            if !editing { template = value }
        }
    }

    /// Sends the template; a blank one goes back to the one in use.
    private func save() {
        let value = template.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !value.isEmpty else {
            template = settings.current.downloads.nameTemplate
            return
        }
        settings.update { $0.downloads.nameTemplate = value }
    }
}
