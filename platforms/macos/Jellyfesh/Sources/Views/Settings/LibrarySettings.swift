// Library, in the order of docs/ui.md, as the web's cards (SettingsCards): the music folders (show,
// remove, add), watching folders for changes, and rescanning with the scan's progress and what the
// last scan did.

import AppKit
import SwiftUI

struct LibrarySettings: View {
    @Environment(LibraryStore.self) private var library
    @Environment(SettingsStore.self) private var settings
    @Environment(\.palette) private var palette
    @State private var removing: String?

    var body: some View {
        SettingsCardsColumn {
            SettingsCard(title: "Music Folders", note: Text("Jellyfesh reads every audio file in these folders.")) {
                if library.roots.isEmpty {
                    Text("No Folders Yet")
                        .lookFont(size: 14)
                        .foregroundStyle(palette[Tokens.Ink.ink70])
                        .padding(.vertical, Tokens.Space.s3)
                }
                ForEach(Array(library.roots.enumerated()), id: \.element) { index, root in
                    if index > 0 {
                        SettingsDivider()
                    }
                    folderRow(root)
                }
                HStack {
                    Spacer()
                    Button("Add Folder…") {
                        FilePanels.chooseMusicFolders { urls in
                            for url in urls { library.addFolder(url) }
                        }
                    }
                }
                .padding(.vertical, Tokens.Space.s3)
            }

            SettingsCard {
                SettingsToggleRow("Watch Folders", help: "Update the library when files change.", isOn: Binding(
                    get: { settings.current.watchFolders },
                    set: { settings.setWatchFolders($0) }
                ))
            }

            SettingsCard(title: "Rescan Library") {
                SettingsRow("Rescan Library", help: "Look for new, changed and removed files.") {
                    Button("Rescan Library") {
                        library.rescan()
                    }
                    .disabled(library.scanning || library.roots.isEmpty)
                }
                scanStatus
            }
        }
        .confirmationDialog(
            Text("Remove “\(removingName)” from the library?"),
            isPresented: Binding(get: { removing != nil }, set: { if !$0 { removing = nil } }),
            titleVisibility: .visible
        ) {
            Button("Remove Folder", role: .destructive) {
                if let root = removing { library.removeFolder(root) }
                removing = nil
            }
        } message: {
            Text("Its songs leave the library and its playlists. The files stay where they are.")
        }
        .task {
            await library.refresh(force: true)
        }
    }

    /// A folder: its name and where it is, Show in Finder and Remove Folder….
    private func folderRow(_ root: String) -> some View {
        SettingsRow(title: Text(verbatim: (root as NSString).lastPathComponent),
                    help: Text(verbatim: (root as NSString).deletingLastPathComponent)) {
            HStack(spacing: Tokens.Space.s2) {
                Button {
                    NSWorkspace.shared.activateFileViewerSelecting([URL(fileURLWithPath: root)])
                } label: {
                    Image(systemName: "magnifyingglass")
                }
                .buttonStyle(.borderless)
                .help(Text("Show in Finder"))
                .accessibilityLabel(Text("Show in Finder"))
                Button("Remove Folder…") {
                    removing = root
                }
            }
        }
        .help(root)
    }

    private var removingName: String {
        guard let removing else { return "" }
        return (removing as NSString).lastPathComponent
    }

    @ViewBuilder private var scanStatus: some View {
        switch library.scan {
        case .progress(let seen, let changed, let current)?:
            SettingsDivider()
            SettingsRow(title: Text("Scanning"), help: Text(verbatim: current)) {
                VStack(alignment: .trailing, spacing: 2) {
                    ProgressView()
                        .controlSize(.small)
                    Text("Files checked: \(seen), changed: \(changed)")
                        .lookFont(size: 12)
                        .monospacedDigit()
                        .foregroundStyle(palette[Tokens.Ink.ink70])
                }
            }
        case .finished(let report)?:
            SettingsDivider()
            SettingsRow(title: Text("Last Scan"),
                        help: Text("Added \(report.added), updated \(report.updated), removed \(report.removed)")) {
                EmptyView()
            }
            if !report.errors.isEmpty {
                DisclosureGroup {
                    ForEach(Array(report.errors.prefix(50).enumerated()), id: \.offset) { _, issue in
                        VStack(alignment: .leading, spacing: 2) {
                            Text((issue.path as NSString).lastPathComponent)
                                .font(.system(size: 12, weight: .semibold))
                            Text(issue.message)
                                .font(.system(size: 11))
                                .foregroundStyle(.secondary)
                        }
                        .frame(maxWidth: .infinity, alignment: .leading)
                        .help(issue.path)
                    }
                } label: {
                    Text("Files that could not be read: \(report.errors.count)")
                }
                .padding(.bottom, Tokens.Space.s3)
            }
        case .failed(let message)?:
            SettingsDivider()
            SettingsRow(title: Text("Last Scan"), help: Text(verbatim: message)) {
                EmptyView()
            }
        case nil:
            EmptyView()
        }
    }
}
