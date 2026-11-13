// Library: the music folders (add, remove, show), rescanning, watching folders for changes, and
// what the last scan did.

import AppKit
import SwiftUI

struct LibrarySettings: View {
    @Environment(LibraryStore.self) private var library
    @Environment(SettingsStore.self) private var settings
    @State private var removing: String?

    var body: some View {
        Form {
            Section {
                if library.roots.isEmpty {
                    Text("No folders yet")
                        .foregroundStyle(.secondary)
                }
                ForEach(library.roots, id: \.self) { root in
                    HStack(spacing: Tokens.Space.s2) {
                        Image(systemName: "folder")
                            .foregroundStyle(.secondary)
                        Text(root)
                            .lineLimit(1)
                            .truncationMode(.middle)
                            .help(root)
                        Spacer()
                        Button {
                            NSWorkspace.shared.activateFileViewerSelecting([URL(fileURLWithPath: root)])
                        } label: {
                            Image(systemName: "magnifyingglass")
                        }
                        .buttonStyle(.borderless)
                        .help(Text("Show in Finder"))
                        .accessibilityLabel(Text("Show in Finder"))
                        Button {
                            removing = root
                        } label: {
                            Image(systemName: "minus.circle")
                        }
                        .buttonStyle(.borderless)
                        .help(Text("Remove Folder"))
                        .accessibilityLabel(Text("Remove Folder"))
                    }
                }
                HStack {
                    Button("Add Folder…") {
                        FilePanels.chooseMusicFolders { urls in
                            for url in urls { library.addFolder(url.path) }
                        }
                    }
                    Spacer()
                    Button("Rescan Library") {
                        library.rescan()
                    }
                    .disabled(library.scanning || library.roots.isEmpty)
                    .help(Text("Look for new, changed and removed files."))
                }
            } header: {
                Text("Music Folders")
            } footer: {
                Text("Jellyfesh reads every audio file in these folders.")
                    .font(.footnote)
                    .foregroundStyle(.secondary)
            }

            Section {
                Toggle(isOn: Binding(get: { settings.current.watchFolders }, set: { settings.setWatchFolders($0) })) {
                    Text("Watch Folders")
                    Text("Update the library when files change.")
                }
                if let stats = library.stats, stats.tracks > 0 {
                    LabeledContent("Library") {
                        Text(CountText.joined([
                            CountText.songs(Int(stats.tracks)),
                            CountText.albums(Int(stats.albums)),
                            CountText.artists(Int(stats.artists)),
                        ]))
                    }
                    LabeledContent("Size") {
                        Text(ByteCountFormatter.string(fromByteCount: Int64(clamping: stats.totalBytes), countStyle: .file))
                    }
                }
                scanStatus
            }
        }
        .formStyle(.grouped)
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

    private var removingName: String {
        guard let removing else { return "" }
        return (removing as NSString).lastPathComponent
    }

    @ViewBuilder private var scanStatus: some View {
        switch library.scan {
        case .progress(let seen, let changed, let current)?:
            LabeledContent("Scanning") {
                VStack(alignment: .trailing, spacing: 2) {
                    ProgressView().controlSize(.small)
                    Text("Files checked: \(seen), changed: \(changed)")
                        .monospacedDigit()
                    Text(current)
                        .lineLimit(1)
                        .truncationMode(.middle)
                        .foregroundStyle(.secondary)
                }
            }
        case .finished(let report)?:
            LabeledContent("Last Scan") {
                Text("Added \(report.added), updated \(report.updated), removed \(report.removed)")
                    .monospacedDigit()
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
                        .help(issue.path)
                    }
                } label: {
                    Text("Files that could not be read: \(report.errors.count)")
                }
            }
        case .failed(let message)?:
            LabeledContent("Last Scan") {
                Text(message)
                    .foregroundStyle(.red)
            }
        case nil:
            EmptyView()
        }
    }
}
